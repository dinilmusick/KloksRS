use kloksrs::graph::{compile_plan, DirectedGraph, GraphError, OrchestrationStep};
use kloksrs::registry::{
    EventQueue, RegistryError, ServiceDefinition, ServiceRegistry, ServiceState,
};

#[test]
fn registry_resolves_transitive_dependencies_with_stable_order() {
    let mut registry = ServiceRegistry::new();
    registry
        .register(ServiceDefinition::new("storage", "1").unwrap())
        .unwrap();
    registry
        .register(ServiceDefinition::new("network", "1").unwrap())
        .unwrap();
    registry
        .register(
            ServiceDefinition::new("midi", "1")
                .unwrap()
                .with_dependency("storage")
                .with_dependency("network")
                .with_capability("midi-output"),
        )
        .unwrap();
    assert_eq!(
        registry.dependency_order("midi").unwrap(),
        vec!["network", "storage", "midi"]
    );
    assert_eq!(registry.discover("midi-output").len(), 1);
    assert_eq!(
        registry.transition("midi", ServiceState::Running),
        Err(RegistryError::InvalidTransition)
    );
    registry
        .transition("midi", ServiceState::Initialized)
        .unwrap();
    assert_eq!(
        registry.transition("midi", ServiceState::Running),
        Err(RegistryError::DependencyNotRunning)
    );
    for id in ["network", "storage"] {
        registry.transition(id, ServiceState::Initialized).unwrap();
        registry.transition(id, ServiceState::Running).unwrap();
    }
    registry.transition("midi", ServiceState::Running).unwrap();
    registry.transition("midi", ServiceState::Stopped).unwrap();
    registry.transition("midi", ServiceState::Disposed).unwrap();
    assert_eq!(registry.discover("midi-output").len(), 0);
    registry.unregister("midi").unwrap();
    assert_eq!(registry.len(), 2);
}

#[test]
fn registry_fails_closed_on_missing_cycles_duplicates_and_dependents() {
    let mut r = ServiceRegistry::new();
    r.register(
        ServiceDefinition::new("a", "1")
            .unwrap()
            .with_dependency("b"),
    )
    .unwrap();
    assert_eq!(
        r.dependency_order("a"),
        Err(RegistryError::MissingDependency)
    );
    r.register(
        ServiceDefinition::new("b", "1")
            .unwrap()
            .with_dependency("a"),
    )
    .unwrap();
    assert_eq!(r.dependency_order("a"), Err(RegistryError::DependencyCycle));
    assert_eq!(
        r.register(ServiceDefinition::new("a", "1").unwrap()),
        Err(RegistryError::DuplicateService)
    );
    assert_eq!(
        r.register(
            ServiceDefinition::new("self", "1")
                .unwrap()
                .with_dependency("self")
        ),
        Err(RegistryError::InvalidDefinition)
    );
    r.transition("a", ServiceState::Disposed).unwrap();
    assert_eq!(r.unregister("a"), Err(RegistryError::HasDependents));
}

#[test]
fn graph_rejects_cycles_and_orphans_without_corrupting_edges() {
    let mut graph = DirectedGraph::new();
    graph.insert("a", 1).unwrap();
    graph.insert("b", 2).unwrap();
    graph.insert("c", 3).unwrap();
    graph.connect("a", "b").unwrap();
    graph.connect("b", "c").unwrap();
    assert_eq!(graph.connect("c", "a"), Err(GraphError::Cycle));
    assert_eq!(graph.connect("unknown", "b"), Err(GraphError::UnknownNode));
    assert_eq!(graph.topological_order().unwrap(), vec!["a", "b", "c"]);
    assert!(graph.disconnect("a", "b"));
    assert_eq!(graph.remove("b"), Some(2));
    assert_eq!(graph.topological_order().unwrap(), vec!["a", "c"]);
}

#[test]
fn event_queue_bounds_and_fifo_are_explicit() {
    let mut queue = EventQueue::new(2).unwrap();
    queue.publish(8).unwrap();
    queue.publish(9).unwrap();
    assert_eq!(queue.publish(10), Err(RegistryError::QueueFull));
    assert_eq!(queue.drain_one(), Some(8));
    assert_eq!(queue.drain_one(), Some(9));
    assert_eq!(queue.drain_one(), None);
}

#[test]
fn orchestration_plan_reuses_registry_and_rejects_missing_services() {
    let mut registry = ServiceRegistry::new();
    registry
        .register(ServiceDefinition::new("service", "1").unwrap())
        .unwrap();
    let steps = vec![
        OrchestrationStep {
            id: "two".into(),
            service_id: "service".into(),
            operation: "read".into(),
            depends_on: vec!["one".into()],
        },
        OrchestrationStep {
            id: "one".into(),
            service_id: "service".into(),
            operation: "write".into(),
            depends_on: vec![],
        },
    ];
    assert_eq!(compile_plan(&steps, &registry).unwrap(), vec!["one", "two"]);
    let mut broken = steps.clone();
    broken[0].service_id = "unknown".into();
    assert_eq!(
        compile_plan(&broken, &registry),
        Err(GraphError::UnknownService)
    );
}
