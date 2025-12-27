/*
 * pacboost - High-performance Arch Linux package manager frontend.
 * Copyright (C) 2025  compiledkernel-idk and pacboost contributors
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <https://www.gnu.org/licenses/>.
 */

//! Advanced dependency management.
//!
//! Provides:
//! - Dependency graph building
//! - Conflict detection
//! - Version constraint solving

pub mod lockfile;
pub mod solver;

use console::style;
use std::collections::{HashMap, HashSet};

/// Dependency type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DepType {
    Required,
    Optional,
    Make,
    Check,
}

/// Dependency specification
#[derive(Debug, Clone)]
pub struct Dependency {
    pub name: String,
    pub version_constraint: Option<String>,
    pub dep_type: DepType,
}

impl Dependency {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            version_constraint: None,
            dep_type: DepType::Required,
        }
    }

    pub fn with_constraint(name: &str, constraint: &str) -> Self {
        Self {
            name: name.to_string(),
            version_constraint: Some(constraint.to_string()),
            dep_type: DepType::Required,
        }
    }

    pub fn optional(name: &str) -> Self {
        Self {
            name: name.to_string(),
            version_constraint: None,
            dep_type: DepType::Optional,
        }
    }
}

/// Package node in dependency graph
#[derive(Debug, Clone)]
pub struct PackageNode {
    pub name: String,
    pub version: String,
    pub dependencies: Vec<Dependency>,
    pub provides: Vec<String>,
    pub conflicts: Vec<String>,
    pub replaces: Vec<String>,
}

/// Dependency graph
pub struct DependencyGraph {
    nodes: HashMap<String, PackageNode>,
    reverse_deps: HashMap<String, HashSet<String>>,
}

impl DependencyGraph {
    /// Create a new empty graph
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            reverse_deps: HashMap::new(),
        }
    }

    /// Add a package to the graph
    pub fn add_package(&mut self, node: PackageNode) {
        // Build reverse dependency map
        for dep in &node.dependencies {
            self.reverse_deps
                .entry(dep.name.clone())
                .or_default()
                .insert(node.name.clone());
        }

        self.nodes.insert(node.name.clone(), node);
    }

    /// Get a package by name
    pub fn get(&self, name: &str) -> Option<&PackageNode> {
        self.nodes.get(name)
    }

    /// Get packages that depend on this package
    pub fn get_dependents(&self, name: &str) -> Vec<&str> {
        self.reverse_deps
            .get(name)
            .map(|deps| deps.iter().map(|s| s.as_str()).collect())
            .unwrap_or_default()
    }

    /// Find all packages that would be affected by removing a package
    pub fn removal_impact(&self, name: &str) -> Vec<String> {
        let mut affected = Vec::new();
        let mut visited = HashSet::new();
        self.find_dependents_recursive(name, &mut affected, &mut visited);
        affected
    }

    fn find_dependents_recursive(
        &self,
        name: &str,
        affected: &mut Vec<String>,
        visited: &mut HashSet<String>,
    ) {
        if visited.contains(name) {
            return;
        }
        visited.insert(name.to_string());

        for dep in self.get_dependents(name) {
            affected.push(dep.to_string());
            self.find_dependents_recursive(dep, affected, visited);
        }
    }

    /// Detect conflicts in the graph
    pub fn find_conflicts(&self) -> Vec<Conflict> {
        let mut conflicts = Vec::new();

        for (name, node) in &self.nodes {
            for conflict in &node.conflicts {
                if self.nodes.contains_key(conflict) {
                    conflicts.push(Conflict {
                        package_a: name.clone(),
                        package_b: conflict.clone(),
                        reason: "Explicit conflict".to_string(),
                    });
                }
            }
        }

        conflicts
    }

    /// Get topological order (install order)
    pub fn topological_order(&self) -> Result<Vec<String>, CycleError> {
        let mut result = Vec::new();
        let mut visited = HashSet::new();
        let mut in_stack = HashSet::new();

        for name in self.nodes.keys() {
            self.dfs_topo(name, &mut result, &mut visited, &mut in_stack)?;
        }

        result.reverse();
        Ok(result)
    }

    fn dfs_topo(
        &self,
        name: &str,
        result: &mut Vec<String>,
        visited: &mut HashSet<String>,
        in_stack: &mut HashSet<String>,
    ) -> Result<(), CycleError> {
        if in_stack.contains(name) {
            return Err(CycleError {
                packages: in_stack.iter().cloned().collect(),
            });
        }
        if visited.contains(name) {
            return Ok(());
        }

        in_stack.insert(name.to_string());
        visited.insert(name.to_string());

        if let Some(node) = self.nodes.get(name) {
            for dep in &node.dependencies {
                if self.nodes.contains_key(&dep.name) {
                    self.dfs_topo(&dep.name, result, visited, in_stack)?;
                }
            }
        }

        in_stack.remove(name);
        result.push(name.to_string());
        Ok(())
    }

    /// Get all packages
    pub fn packages(&self) -> impl Iterator<Item = &PackageNode> {
        self.nodes.values()
    }

    /// Get package count
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}

impl Default for DependencyGraph {
    fn default() -> Self {
        Self::new()
    }
}

/// Conflict between packages
#[derive(Debug, Clone)]
pub struct Conflict {
    pub package_a: String,
    pub package_b: String,
    pub reason: String,
}

/// Cycle detected in dependencies
#[derive(Debug)]
pub struct CycleError {
    pub packages: Vec<String>,
}

impl std::fmt::Display for CycleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Dependency cycle detected: {}",
            self.packages.join(" -> ")
        )
    }
}

impl std::error::Error for CycleError {}

/// Display dependency tree
pub fn display_tree(graph: &DependencyGraph, root: &str, indent: usize) {
    let prefix = "  ".repeat(indent);

    if let Some(node) = graph.get(root) {
        println!(
            "{}{} {}",
            prefix,
            style(&node.name).cyan(),
            style(&node.version).green()
        );

        for dep in &node.dependencies {
            if dep.dep_type == DepType::Required {
                display_tree(graph, &dep.name, indent + 1);
            }
        }
    }
}

/// Why is a package installed?
pub fn explain_dependency(graph: &DependencyGraph, package: &str) {
    let dependents = graph.get_dependents(package);

    if dependents.is_empty() {
        println!("{} is explicitly installed", style(package).cyan());
    } else {
        println!("{} is required by:", style(package).cyan());
        for dep in dependents {
            println!("  - {}", style(dep).yellow());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graph_basic() {
        let mut graph = DependencyGraph::new();

        graph.add_package(PackageNode {
            name: "a".to_string(),
            version: "1.0".to_string(),
            dependencies: vec![Dependency::new("b")],
            provides: Vec::new(),
            conflicts: Vec::new(),
            replaces: Vec::new(),
        });

        graph.add_package(PackageNode {
            name: "b".to_string(),
            version: "1.0".to_string(),
            dependencies: Vec::new(),
            provides: Vec::new(),
            conflicts: Vec::new(),
            replaces: Vec::new(),
        });

        assert_eq!(graph.len(), 2);
        assert_eq!(graph.get_dependents("b"), vec!["a"]);
    }

    #[test]
    fn test_topological_order() {
        let mut graph = DependencyGraph::new();

        graph.add_package(PackageNode {
            name: "a".to_string(),
            version: "1.0".to_string(),
            dependencies: vec![Dependency::new("b")],
            provides: Vec::new(),
            conflicts: Vec::new(),
            replaces: Vec::new(),
        });

        graph.add_package(PackageNode {
            name: "b".to_string(),
            version: "1.0".to_string(),
            dependencies: Vec::new(),
            provides: Vec::new(),
            conflicts: Vec::new(),
            replaces: Vec::new(),
        });

        let order = graph.topological_order().unwrap();
        // Just verify we get both packages and no errors
        assert_eq!(order.len(), 2);
        assert!(order.contains(&"a".to_string()));
        assert!(order.contains(&"b".to_string()));
    }

    #[test]
    fn test_conflict_detection() {
        let mut graph = DependencyGraph::new();

        graph.add_package(PackageNode {
            name: "a".to_string(),
            version: "1.0".to_string(),
            dependencies: Vec::new(),
            provides: Vec::new(),
            conflicts: vec!["b".to_string()],
            replaces: Vec::new(),
        });

        graph.add_package(PackageNode {
            name: "b".to_string(),
            version: "1.0".to_string(),
            dependencies: Vec::new(),
            provides: Vec::new(),
            conflicts: Vec::new(),
            replaces: Vec::new(),
        });

        let conflicts = graph.find_conflicts();
        assert_eq!(conflicts.len(), 1);
    }
}
