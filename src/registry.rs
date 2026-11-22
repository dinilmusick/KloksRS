//! Allocation-only service definitions and deterministic dependency resolution.
//! Canonical inventory: Kloks@286377a03a8c5973c592b61ccaaa3864d060e8cc.

use alloc::collections::{BTreeMap, BTreeSet, VecDeque};
use alloc::string::{String, ToString};
use alloc::vec::Vec;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RegistryError {
    InvalidDefinition,
    DuplicateService,
    UnknownService,
    MissingDependency,
    DependencyCycle,
    DependencyNotRunning,
    InvalidTransition,
    HasDependents,
    QueueFull,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServiceDefinition {
    pub id: String,
    pub version: String,
    pub capabilities: BTreeSet<String>,
    pub dependencies: BTreeSet<String>,
}

impl ServiceDefinition {
    pub fn new(id: impl Into<String>, version: impl Into<String>) -> Result<Self, RegistryError> {
        let id = id.into();
        let version = version.into();
        if id.trim().is_empty() || version.trim().is_empty() {
            return Err(RegistryError::InvalidDefinition);
        }
        Ok(Self {
            id,
            version,
            capabilities: BTreeSet::new(),
            dependencies: BTreeSet::new(),
        })
    }

    pub fn with_capability(mut self, capability: impl Into<String>) -> Self {
        self.capabilities.insert(capability.into());
        self
    }

    pub fn with_dependency(mut self, dependency: impl Into<String>) -> Self {
        self.dependencies.insert(dependency.into());
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServiceState {
    Registered,
    Initialized,
    Running,
    Stopped,
    Disposed,
}

#[derive(Debug, Default)]
pub struct ServiceRegistry {
    definitions: BTreeMap<String, ServiceDefinition>,
    states: BTreeMap<String, ServiceState>,
}

impl ServiceRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, definition: ServiceDefinition) -> Result<(), RegistryError> {
        if self.definitions.contains_key(&definition.id) {
            return Err(RegistryError::DuplicateService);
        }
        if definition.dependencies.contains(&definition.id)
            || definition.capabilities.iter().any(|c| c.trim().is_empty())
            || definition.dependencies.iter().any(|d| d.trim().is_empty())
        {
            return Err(RegistryError::InvalidDefinition);
        }
        self.states.insert(definition.id.clone(), ServiceState::Registered);
        self.definitions.insert(definition.id.clone(), definition);
        Ok(())
    }

    pub fn definition(&self, id: &str) -> Option<&ServiceDefinition> {
        self.definitions.get(id)
    }

    pub fn state(&self, id: &str) -> Result<ServiceState, RegistryError> {
        self.states.get(id).copied().ok_or(RegistryError::UnknownService)
    }

    pub fn discover(&self, capability: &str) -> Vec<&ServiceDefinition> {
        self.definitions
            .values()
            .filter(|d| d.capabilities.contains(capability)
                && self.states.get(&d.id) != Some(&ServiceState::Disposed))
            .collect()
    }

    fn visit(
        &self, id: &str, active: &mut BTreeSet<String>,
        complete: &mut BTreeSet<String>, out: &mut Vec<String>,
    ) -> Result<(), RegistryError> {
        if complete.contains(id) {
            return Ok(());
        }
        if !active.insert(id.to_string()) {
            return Err(RegistryError::DependencyCycle);
        }
        let definition = self.definitions.get(id).ok_or(RegistryError::MissingDependency)?;
        if self.states.get(id) == Some(&ServiceState::Disposed) {
            return Err(RegistryError::InvalidTransition);
        }
        for dependency in &definition.dependencies {
            self.visit(dependency, active, complete, out)?;
        }
        active.remove(id);
        complete.insert(id.to_string());
        out.push(id.to_string());
        Ok(())
    }

    /// Dependencies precede their consumers; deterministic lexical tie-breaking.
    pub fn dependency_order(&self, id: &str) -> Result<Vec<String>, RegistryError> {
        if !self.definitions.contains_key(id) {
            return Err(RegistryError::UnknownService);
        }
        let mut active = BTreeSet::new();
        let mut complete = BTreeSet::new();
        let mut order = Vec::new();
        self.visit(id, &mut active, &mut complete, &mut order)?;
        Ok(order)
    }

    /// Caller controls lifecycle; no implicit asynchronous startup or scheduling.
    pub fn transition(&mut self, id: &str, next: ServiceState) -> Result<(), RegistryError> {
        use ServiceState::{Disposed, Initialized, Registered, Running, Stopped};
        let previous = self.state(id)?;
        let allowed = matches!(
            (previous, next),
            (Registered, Initialized) | (Initialized, Running)
                | (Stopped, Running) | (Running, Stopped)
                | (Stopped, Initialized) | (Registered, Disposed)
                | (Initialized, Disposed) | (Stopped, Disposed)
        );
        if !allowed {
            return Err(RegistryError::InvalidTransition);
        }
        if next == Running {
            let definition = self.definitions.get(id).ok_or(RegistryError::UnknownService)?;
            for dependency in &definition.dependencies {
                if self.state(dependency)? != Running {
                    return Err(RegistryError::DependencyNotRunning);
                }
            }
        }
        self.states.insert(id.to_string(), next);
        Ok(())
    }

    pub fn unregister(&mut self, id: &str) -> Result<(), RegistryError> {
        if self.state(id)? != ServiceState::Disposed {
            return Err(RegistryError::InvalidTransition);
        }
        if self.definitions.values().any(|d| d.id != id && d.dependencies.contains(id)) {
            return Err(RegistryError::HasDependents);
        }
        self.definitions.remove(id);
        self.states.remove(id);
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.definitions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.definitions.is_empty()
    }
}

/// A bounded, host-drained queue; no background dispatcher or hidden threads.
pub struct EventQueue<T> {
    capacity: usize,
    items: VecDeque<T>,
}

impl<T> EventQueue<T> {
    pub fn new(capacity: usize) -> Result<Self, RegistryError> {
        if capacity == 0 {
            return Err(RegistryError::InvalidDefinition);
        }
        Ok(Self { capacity, items: VecDeque::new() })
    }

    pub fn publish(&mut self, item: T) -> Result<(), RegistryError> {
        if self.items.len() == self.capacity {
            return Err(RegistryError::QueueFull);
        }
        self.items.push_back(item);
        Ok(())
    }

    pub fn drain_one(&mut self) -> Option<T> {
        self.items.pop_front()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}
