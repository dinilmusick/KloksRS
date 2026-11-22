//! Portable state and orchestration dependency graph without rendering ownership.
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use crate::registry::ServiceRegistry;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GraphError {
    InvalidNode,
    DuplicateNode,
    UnknownNode,
    DuplicateEdge,
    Cycle,
    UnknownService,
    InvalidStep,
}

pub struct DirectedGraph<T> {
    nodes: BTreeMap<String, T>,
    edges: BTreeSet<(String, String)>,
}

impl<T> Default for DirectedGraph<T> {
    fn default() -> Self {
        Self { nodes: BTreeMap::new(), edges: BTreeSet::new() }
    }
}

impl<T> DirectedGraph<T> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, id: impl Into<String>, value: T) -> Result<(), GraphError> {
        let id = id.into();
        if id.trim().is_empty() {
            return Err(GraphError::InvalidNode);
        }
        if self.nodes.contains_key(&id) {
            return Err(GraphError::DuplicateNode);
        }
        self.nodes.insert(id, value);
        Ok(())
    }

    pub fn get(&self, id: &str) -> Option<&T> {
        self.nodes.get(id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut T> {
        self.nodes.get_mut(id)
    }

    pub fn connect(&mut self, from: &str, to: &str) -> Result<(), GraphError> {
        if !self.nodes.contains_key(from) || !self.nodes.contains_key(to) {
            return Err(GraphError::UnknownNode);
        }
        let edge = (from.to_string(), to.to_string());
        if !self.edges.insert(edge.clone()) {
            return Err(GraphError::DuplicateEdge);
        }
        if self.topological_order().is_err() {
            self.edges.remove(&edge);
            return Err(GraphError::Cycle);
        }
        Ok(())
    }

    pub fn disconnect(&mut self, from: &str, to: &str) -> bool {
        self.edges.remove(&(from.to_string(), to.to_string()))
    }

    pub fn remove(&mut self, id: &str) -> Option<T> {
        self.edges.retain(|(a, b)| a != id && b != id);
        self.nodes.remove(id)
    }

    pub fn topological_order(&self) -> Result<Vec<String>, GraphError> {
        let mut degree = self.nodes.keys().map(|id| (id.clone(), 0usize))
            .collect::<BTreeMap<_, _>>();
        for (_, target) in &self.edges {
            *degree.get_mut(target).ok_or(GraphError::UnknownNode)? += 1;
        }
        let mut ready = degree.iter().filter(|(_, count)| **count == 0)
            .map(|(id, _)| id.clone()).collect::<BTreeSet<_>>();
        let mut out = Vec::new();
        while let Some(next) = ready.iter().next().cloned() {
            ready.remove(&next);
            out.push(next.clone());
            for (from, to) in &self.edges {
                if from == &next {
                    let count = degree.get_mut(to).ok_or(GraphError::UnknownNode)?;
                    *count -= 1;
                    if *count == 0 {
                        ready.insert(to.clone());
                    }
                }
            }
        }
        if out.len() != self.nodes.len() {
            return Err(GraphError::Cycle);
        }
        Ok(out)
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}

/// An explicit host-executed orchestration step. KrnlRS owns the runtime.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrchestrationStep {
    pub id: String,
    pub service_id: String,
    pub operation: String,
    pub depends_on: Vec<String>,
}

pub fn compile_plan(
    steps: &[OrchestrationStep],
    services: &ServiceRegistry,
) -> Result<Vec<String>, GraphError> {
    let mut graph = DirectedGraph::new();
    for step in steps {
        if step.id.trim().is_empty() || step.operation.trim().is_empty() {
            return Err(GraphError::InvalidStep);
        }
        if services.definition(&step.service_id).is_none() {
            return Err(GraphError::UnknownService);
        }
        graph.insert(step.id.clone(), ())?;
    }
    for step in steps {
        for dependency in &step.depends_on {
            graph.connect(dependency, &step.id)?;
        }
    }
    graph.topological_order()
}
