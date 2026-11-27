#![deny(warnings)]
//! Graph analysis using petgraph for blast radius, cycles, and topological queries.

use petgraph::Direction;
use petgraph::algo::{is_cyclic_directed, toposort};
use petgraph::graph::{DiGraph, NodeIndex};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct KnowledgeGraph {
    nodes: Vec<String>,
    edges: Vec<(String, String, String)>,
}

pub struct DependencyGraph {
    graph: DiGraph<String, String>,
    node_indices: HashMap<String, NodeIndex>,
}

impl DependencyGraph {
    pub fn new() -> Self {
        Self {
            graph: DiGraph::new(),
            node_indices: HashMap::new(),
        }
    }

    pub fn get_or_insert_node(&mut self, name: &str) -> NodeIndex {
        if let Some(&idx) = self.node_indices.get(name) {
            idx
        } else {
            let idx = self.graph.add_node(name.to_string());
            self.node_indices.insert(name.to_string(), idx);
            idx
        }
    }

    pub fn add_edge(&mut self, from: &str, to: &str, kind: &str) {
        let from_idx = self.get_or_insert_node(from);
        let to_idx = self.get_or_insert_node(to);
        self.graph.add_edge(from_idx, to_idx, kind.to_string());
    }

    pub fn from_edges<I, S1, S2>(edges: I) -> Self
    where
        I: IntoIterator<Item = (S1, S2)>,
        S1: AsRef<str>,
        S2: AsRef<str>,
    {
        let mut graph = Self::new();
        for (from, to) in edges {
            graph.add_edge(from.as_ref(), to.as_ref(), "import");
        }
        graph
    }

    /// Calculates the blast radius (all downstream dependent files) if a file is changed.
    /// Traverses incoming edges (dependents).
    pub fn blast_radius(&self, target_file: &str, max_depth: Option<usize>) -> Vec<String> {
        let Some(&start_idx) = self.node_indices.get(target_file) else {
            return Vec::new();
        };

        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        queue.push_back((start_idx, 0));
        visited.insert(start_idx);

        let mut affected = Vec::new();

        while let Some((curr, depth)) = queue.pop_front() {
            if let Some(limit) = max_depth
                && depth >= limit
            {
                continue;
            }
            // Incoming edges mean nodes that import `curr`
            for neighbor in self.graph.neighbors_directed(curr, Direction::Incoming) {
                if visited.insert(neighbor) {
                    affected.push(self.graph[neighbor].clone());
                    queue.push_back((neighbor, depth + 1));
                }
            }
        }

        affected.sort();
        affected
    }

    /// Checks if there are circular dependencies in the graph.
    pub fn has_cycles(&self) -> bool {
        is_cyclic_directed(&self.graph)
    }

    /// Returns topological order of files (or error if cycles exist).
    pub fn topological_order(&self) -> Result<Vec<String>, String> {
        match toposort(&self.graph, None) {
            Ok(indices) => Ok(indices
                .into_iter()
                .map(|idx| self.graph[idx].clone())
                .collect()),
            Err(_) => Err("Graph contains circular dependencies".to_string()),
        }
    }
}

impl Default for DependencyGraph {
    fn default() -> Self {
        Self::new()
    }
}
