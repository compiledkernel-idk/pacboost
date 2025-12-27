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

//! AUR dependency resolution with topological sorting.

use anyhow::{anyhow, Result};
use std::collections::{HashMap, HashSet, VecDeque};

use super::client::{parse_dependency, AurClient, AurPackageInfo};
use crate::error::PacboostError;

/// Package source type
#[derive(Debug, Clone, PartialEq)]
pub enum PackageSource {
    /// Package is in official Arch repositories
    Official { repo: String },
    /// Package is in AUR
    Aur { votes: u32, popularity: f64 },
    /// Package is installed locally
    Installed,
    /// Package provides a virtual package
    Virtual { provider: String },
}

/// Node in the dependency graph
#[derive(Debug, Clone)]
pub struct PackageNode {
    pub name: String,
    pub version: String,
    pub source: PackageSource,
    pub info: Option<AurPackageInfo>,
}

/// Dependency graph for AUR packages with topological sorting
pub struct DependencyGraph {
    /// All nodes in the graph
    nodes: HashMap<String, PackageNode>,
    /// Edges: package -> packages it depends on
    edges: HashMap<String, Vec<String>>,
    /// Reverse edges: package -> packages that depend on it
    reverse_edges: HashMap<String, Vec<String>>,
}

impl DependencyGraph {
    /// Create a new empty dependency graph
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            edges: HashMap::new(),
            reverse_edges: HashMap::new(),
        }
    }

    /// Build a complete dependency graph from target packages
    ///
    /// This performs BFS to discover all AUR dependencies, checking each
    /// dependency against the official repos first.
    pub async fn build(
        targets: Vec<String>,
        client: &AurClient,
        official_check: impl Fn(&str) -> bool,
    ) -> Result<Self> {
        let mut graph = Self::new();
        let mut queue: VecDeque<String> = targets.clone().into_iter().collect();
        let mut visited: HashSet<String> = HashSet::new();
        let targets_set: HashSet<String> = targets.into_iter().collect();

        while !queue.is_empty() {
            let mut current_layer = Vec::new();
            while let Some(pkg) = queue.pop_front() {
                if !visited.contains(&pkg) {
                    visited.insert(pkg.clone());
                    // Check official/local first
                    if official_check(&pkg) {
                        graph.add_official_node(&pkg, "sync");
                    } else {
                        current_layer.push(pkg);
                    }
                }
            }

            if current_layer.is_empty() {
                break;
            }

            // Fetch the whole layer in parallel via batch RPC
            match client.get_info_batch(&current_layer).await {
                Ok(results) => {
                    let mut found_names = HashSet::new();
                    for info in results {
                        found_names.insert(info.name.clone());
                        graph.add_aur_node(info.clone());

                        // Queue dependencies
                        let deps: Vec<String> = info
                            .all_deps()
                            .into_iter()
                            .map(|d| parse_dependency(&d).0)
                            .collect();

                        graph.edges.insert(info.name.clone(), deps.clone());
                        for dep in &deps {
                            graph
                                .reverse_edges
                                .entry(dep.clone())
                                .or_default()
                                .push(info.name.clone());
                            if !visited.contains(dep) {
                                queue.push_back(dep.clone());
                            }
                        }
                    }

                    // Check for missing targets
                    for pkg in current_layer {
                        if !found_names.contains(&pkg) && targets_set.contains(&pkg) {
                            return Err(anyhow!(
                                "Package '{}' not found in AUR or official repositories",
                                pkg
                            ));
                        } else if !found_names.contains(&pkg) {
                            // Dependency not found (likely virtual)
                            graph.add_official_node(&pkg, "virtual");
                        }
                    }
                }
                Err(e) => return Err(e),
            }
        }

        Ok(graph)
    }

    /// Add an official package node
    fn add_official_node(&mut self, name: &str, repo: &str) {
        self.nodes.insert(
            name.to_string(),
            PackageNode {
                name: name.to_string(),
                version: String::new(),
                source: PackageSource::Official {
                    repo: repo.to_string(),
                },
                info: None,
            },
        );
    }

    /// Add an AUR package node
    fn add_aur_node(&mut self, info: AurPackageInfo) {
        self.nodes.insert(
            info.name.clone(),
            PackageNode {
                name: info.name.clone(),
                version: info.version.clone(),
                source: PackageSource::Aur {
                    votes: info.num_votes,
                    popularity: info.popularity,
                },
                info: Some(info),
            },
        );
    }

    /// Perform topological sort to get installation order
    ///
    /// Returns packages in order where dependencies come before dependents.
    /// Returns an error if a circular dependency is detected.
    pub fn topological_sort(&self) -> Result<Vec<String>> {
        let mut sorted = Vec::new();
        let mut visited: HashSet<String> = HashSet::new();
        let mut temp_mark: HashSet<String> = HashSet::new();
        let mut cycle_path: Vec<String> = Vec::new();

        for node_name in self.nodes.keys() {
            if !visited.contains(node_name) {
                self.visit_node(
                    node_name,
                    &mut visited,
                    &mut temp_mark,
                    &mut sorted,
                    &mut cycle_path,
                )?;
            }
        }

        // No reverse needed - our edge semantics (A depends on B = edge A->B)
        // means dependencies are naturally added before dependents in post-order DFS
        Ok(sorted)
    }

    /// DFS visit for topological sort
    fn visit_node(
        &self,
        node: &str,
        visited: &mut HashSet<String>,
        temp_mark: &mut HashSet<String>,
        sorted: &mut Vec<String>,
        cycle_path: &mut Vec<String>,
    ) -> Result<()> {
        if temp_mark.contains(node) {
            // Circular dependency detected
            cycle_path.push(node.to_string());
            let cycle_start = cycle_path.iter().position(|n| n == node).unwrap_or(0);
            let cycle: Vec<String> = cycle_path[cycle_start..].to_vec();

            return Err(PacboostError::CircularDependency { cycle }.into());
        }

        if visited.contains(node) {
            return Ok(());
        }

        temp_mark.insert(node.to_string());
        cycle_path.push(node.to_string());

        if let Some(deps) = self.edges.get(node) {
            for dep in deps {
                if self.nodes.contains_key(dep) {
                    self.visit_node(dep, visited, temp_mark, sorted, cycle_path)?;
                }
            }
        }

        temp_mark.remove(node);
        cycle_path.pop();
        visited.insert(node.to_string());
        sorted.push(node.to_string());

        Ok(())
    }

    /// Get only AUR packages in installation order
    pub fn aur_packages_sorted(&self) -> Result<Vec<&AurPackageInfo>> {
        let sorted = self.topological_sort()?;

        Ok(sorted
            .iter()
            .filter_map(|name| self.nodes.get(name).and_then(|node| node.info.as_ref()))
            .collect())
    }

    /// Get all official dependencies
    pub fn official_deps(&self) -> Vec<String> {
        self.nodes
            .iter()
            .filter(|(_, node)| matches!(node.source, PackageSource::Official { .. }))
            .map(|(name, _)| name.clone())
            .collect()
    }

    /// Get packages that depend on a given package
    pub fn dependents(&self, package: &str) -> Vec<String> {
        self.reverse_edges.get(package).cloned().unwrap_or_default()
    }

    /// Check if a package is in the graph
    pub fn contains(&self, package: &str) -> bool {
        self.nodes.contains_key(package)
    }

    /// Get the number of packages in the graph
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Check if the graph is empty
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Get detailed info for a package
    pub fn get_info(&self, package: &str) -> Option<&AurPackageInfo> {
        self.nodes.get(package).and_then(|node| node.info.as_ref())
    }

    /// Detect potential conflicts between packages
    pub fn find_conflicts(&self) -> Vec<(String, String, String)> {
        let mut conflicts = Vec::new();

        for (name, node) in &self.nodes {
            if let Some(info) = &node.info {
                for conflict in &info.conflicts {
                    let (conflict_name, _) = parse_dependency(conflict);
                    if self.nodes.contains_key(&conflict_name) {
                        conflicts.push((
                            name.clone(),
                            conflict_name,
                            format!("{} conflicts with dependency", name),
                        ));
                    }
                }
            }
        }

        conflicts
    }

    /// Get packages providing a virtual package
    pub fn find_providers(&self, virtual_pkg: &str) -> Vec<String> {
        self.nodes
            .iter()
            .filter(|(_, node)| {
                if let Some(info) = &node.info {
                    info.provides.iter().any(|p| {
                        let (pname, _) = parse_dependency(p);
                        pname == virtual_pkg
                    })
                } else {
                    false
                }
            })
            .map(|(name, _)| name.clone())
            .collect()
    }
}

impl Default for DependencyGraph {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_graph() {
        let graph = DependencyGraph::new();
        assert!(graph.is_empty());
        assert_eq!(graph.len(), 0);
    }

    #[test]
    fn test_topological_sort_simple() {
        let mut graph = DependencyGraph::new();

        // A depends on B, B depends on C
        graph.nodes.insert(
            "a".to_string(),
            PackageNode {
                name: "a".to_string(),
                version: "1.0".to_string(),
                source: PackageSource::Aur {
                    votes: 0,
                    popularity: 0.0,
                },
                info: None,
            },
        );
        graph.nodes.insert(
            "b".to_string(),
            PackageNode {
                name: "b".to_string(),
                version: "1.0".to_string(),
                source: PackageSource::Aur {
                    votes: 0,
                    popularity: 0.0,
                },
                info: None,
            },
        );
        graph.nodes.insert(
            "c".to_string(),
            PackageNode {
                name: "c".to_string(),
                version: "1.0".to_string(),
                source: PackageSource::Aur {
                    votes: 0,
                    popularity: 0.0,
                },
                info: None,
            },
        );

        graph.edges.insert("a".to_string(), vec!["b".to_string()]);
        graph.edges.insert("b".to_string(), vec!["c".to_string()]);
        graph.edges.insert("c".to_string(), vec![]);

        let sorted = graph.topological_sort().unwrap();

        // c should come before b, b should come before a
        let pos_a = sorted.iter().position(|x| x == "a").unwrap();
        let pos_b = sorted.iter().position(|x| x == "b").unwrap();
        let pos_c = sorted.iter().position(|x| x == "c").unwrap();

        assert!(pos_c < pos_b);
        assert!(pos_b < pos_a);
    }

    #[test]
    fn test_circular_dependency_detection() {
        let mut graph = DependencyGraph::new();

        // A -> B -> C -> A (circular)
        for name in &["a", "b", "c"] {
            graph.nodes.insert(
                name.to_string(),
                PackageNode {
                    name: name.to_string(),
                    version: "1.0".to_string(),
                    source: PackageSource::Aur {
                        votes: 0,
                        popularity: 0.0,
                    },
                    info: None,
                },
            );
        }

        graph.edges.insert("a".to_string(), vec!["b".to_string()]);
        graph.edges.insert("b".to_string(), vec!["c".to_string()]);
        graph.edges.insert("c".to_string(), vec!["a".to_string()]);

        let result = graph.topological_sort();
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(err.to_string().contains("Circular dependency"));
    }

    #[test]
    fn test_diamond_dependency() {
        let mut graph = DependencyGraph::new();

        // Diamond: A -> B, A -> C, B -> D, C -> D
        for name in &["a", "b", "c", "d"] {
            graph.nodes.insert(
                name.to_string(),
                PackageNode {
                    name: name.to_string(),
                    version: "1.0".to_string(),
                    source: PackageSource::Aur {
                        votes: 0,
                        popularity: 0.0,
                    },
                    info: None,
                },
            );
        }

        graph
            .edges
            .insert("a".to_string(), vec!["b".to_string(), "c".to_string()]);
        graph.edges.insert("b".to_string(), vec!["d".to_string()]);
        graph.edges.insert("c".to_string(), vec!["d".to_string()]);
        graph.edges.insert("d".to_string(), vec![]);

        let sorted = graph.topological_sort().unwrap();

        // d should come first, then b and c, then a
        let pos_a = sorted.iter().position(|x| x == "a").unwrap();
        let pos_b = sorted.iter().position(|x| x == "b").unwrap();
        let pos_c = sorted.iter().position(|x| x == "c").unwrap();
        let pos_d = sorted.iter().position(|x| x == "d").unwrap();

        assert!(pos_d < pos_b);
        assert!(pos_d < pos_c);
        assert!(pos_b < pos_a);
        assert!(pos_c < pos_a);
    }
}
