//! `astar.AStar` with `astar.datastructures.OpenSet` (a `java.util.PriorityQueue` ordered by `Double.compare` of
//! `f()`) and `ClosedSetHash`, over nodes held in an arena (a node is its index; `keyCode()` and identity coincide).
use std::cmp::Ordering;
use std::collections::HashSet;

/// astar.ISearchNode (+ astar.ASearchNode, astar.IGoalNode) over the nodes of one search.
pub trait AStarNodes {
    fn g(&self, n: usize) -> f64;
    fn set_g(&mut self, n: usize, g: f64);
    fn h(&self, n: usize) -> f64;
    /// astar.ASearchNode.f
    fn f(&self, n: usize) -> f64 {
        self.g(n) + self.h(n)
    }
    /// ISearchNode.c: the cost of the step from `from` to its successor `to`.
    fn c(&self, from: usize, to: usize) -> f64;
    /// ISearchNode.getSuccessors
    fn get_successors(&mut self, n: usize, out: &mut Vec<usize>) -> Result<(), String>;
    fn parent(&self, n: usize) -> Option<usize>;
    fn set_parent(&mut self, n: usize, p: Option<usize>);
    fn depth(&self, n: usize) -> i32;
    fn set_depth(&mut self, n: usize, d: i32);
    /// IGoalNode.inGoal
    fn in_goal(&self, n: usize) -> bool;
}

/// `Double.compare`
fn compare(a: f64, b: f64) -> Ordering {
    a.total_cmp(&b)
}

/// java.util.PriorityQueue with the `Double.compare(f, f)` comparator of `AStar.SearchNodeComparator`.
#[derive(Default)]
struct OpenSet {
    q: Vec<usize>,
}

impl OpenSet {
    fn cmp<G: AStarNodes>(g: &G, a: usize, b: usize) -> Ordering {
        compare(g.f(a), g.f(b))
    }

    /// PriorityQueue.siftUpUsingComparator
    fn sift_up<G: AStarNodes>(&mut self, g: &G, mut k: usize, x: usize) {
        while k > 0 {
            let parent = (k - 1) >> 1;
            let e = self.q[parent];
            if Self::cmp(g, x, e) != Ordering::Less {
                break;
            }
            self.q[k] = e;
            k = parent;
        }
        self.q[k] = x;
    }

    /// PriorityQueue.siftDownUsingComparator
    fn sift_down<G: AStarNodes>(&mut self, g: &G, mut k: usize, x: usize, n: usize) {
        let half = n >> 1;
        while k < half {
            let mut child = (k << 1) + 1;
            let mut c = self.q[child];
            let right = child + 1;
            if right < n && Self::cmp(g, c, self.q[right]) == Ordering::Greater {
                child = right;
                c = self.q[child];
            }
            if Self::cmp(g, x, c) != Ordering::Greater {
                break;
            }
            self.q[k] = c;
            k = child;
        }
        self.q[k] = x;
    }

    /// astar.datastructures.OpenSet.add: PriorityQueue.offer
    fn add<G: AStarNodes>(&mut self, g: &G, node: usize) {
        let i = self.q.len();
        self.q.push(node);
        self.sift_up(g, i, node);
    }

    /// astar.datastructures.OpenSet.poll: PriorityQueue.poll
    fn poll<G: AStarNodes>(&mut self, g: &G) -> Option<usize> {
        if self.q.is_empty() {
            return None;
        }
        let result = self.q[0];
        let n = self.q.len() - 1;
        let x = self.q.pop().unwrap_or(result);
        if n > 0 {
            self.sift_down(g, 0, x, n);
        }
        Some(result)
    }

    /// astar.datastructures.OpenSet.remove: PriorityQueue.remove(Object) = removeAt(indexOf)
    fn remove<G: AStarNodes>(&mut self, g: &G, node: usize) {
        let Some(i) = self.q.iter().position(|&x| x == node) else {
            return;
        };
        let s = self.q.len() - 1;
        if s == i {
            self.q.pop();
        } else {
            let moved = self.q[s];
            self.q.pop();
            self.sift_down(g, i, moved, s);
            if self.q[i] == moved {
                self.sift_up(g, i, moved);
            }
        }
    }

    fn contains(&self, node: usize) -> bool {
        self.q.contains(&node)
    }
}

/// astar.AStar.search: the goal node reached, or `None` when the open set ran dry or `max_steps` (negative: no
/// limit) was used up.
#[allow(clippy::neg_cmp_op_on_partial_ord)]
pub fn search<G: AStarNodes>(
    g: &mut G,
    initial: usize,
    max_steps: i32,
) -> Result<Option<usize>, String> {
    g.set_depth(initial, 0);
    let mut open = OpenSet::default();
    open.add(g, initial);
    let mut closed: HashSet<usize> = HashSet::new();
    let mut steps = 0;
    let mut successors: Vec<usize> = Vec::new();
    while !open.q.is_empty() && (max_steps < 0 || steps < max_steps) {
        let Some(current) = open.poll(g) else { break };
        if g.in_goal(current) {
            return Ok(Some(current));
        }
        successors.clear();
        g.get_successors(current, &mut successors)?;
        for &successor in &successors {
            if Some(successor) != g.parent(current) && !closed.contains(&successor) {
                let in_open = open.contains(successor);
                let tentative = g.g(current) + g.c(current, successor);
                if !in_open || !(tentative >= g.g(successor)) {
                    g.set_parent(successor, Some(current));
                    let d = g.depth(current) + 1;
                    g.set_depth(successor, d);
                    if in_open {
                        open.remove(g, successor);
                        g.set_g(successor, tentative);
                        open.add(g, successor);
                    } else {
                        g.set_g(successor, tentative);
                        open.add(g, successor);
                    }
                }
            }
        }
        closed.insert(current);
        steps += 1;
    }
    Ok(None)
}

/// astar.AStar.path: the nodes from the start to `node`; the original throws "circular path?" past 5000 nodes.
pub fn path<G: AStarNodes>(g: &G, node: usize) -> Result<Vec<usize>, String> {
    let mut p = vec![node];
    let mut current = node;
    while let Some(parent) = g.parent(current) {
        p.insert(0, parent);
        if p.len() > 5000 {
            return Err("circular path?".into());
        }
        current = parent;
    }
    Ok(p)
}

/// The native A* (`libPZPathFind` `AStar::search`) keeps its open list in Detour's `dtNodeQueue`, a binary heap on the
/// float `total` of a node with `bubbleUp`/`trickleDown`; a node's state is two flag bits (open, closed).
#[derive(Default)]
struct NativeQueue {
    heap: Vec<usize>,
}

/// dtNode.total and dtNode.flags of the nodes of one search; a node starts with `total` 0 and no flag
/// (`SearchNode::init`).
#[derive(Default)]
struct NativeNodes {
    total: Vec<f32>,
    flags: Vec<u8>,
}

impl NativeNodes {
    fn ensure(&mut self, n: usize) {
        if self.total.len() <= n {
            self.total.resize(n + 1, 0.0);
            self.flags.resize(n + 1, 0);
        }
    }
}

impl NativeQueue {
    /// dtNodeQueue::bubbleUp(i, node): the parent moves down while its total is greater than the node's.
    fn bubble_up(&mut self, nodes: &NativeNodes, mut i: usize, node: usize) {
        while i > 0 {
            let parent = (i - 1) >> 1;
            let p = self.heap[parent];
            // UCOMISS + JBE: stop unless parent.total > node.total (an unordered compare stops too)
            if nodes.total[p] > nodes.total[node] {
                self.heap[i] = p;
                i = parent;
            } else {
                break;
            }
        }
        self.heap[i] = node;
    }

    /// dtNodeQueue::trickleDown(i, node): the smaller child (the left one on a tie) moves up all the way to the
    /// bottom, then the node bubbles up from there.
    fn trickle_down(&mut self, nodes: &NativeNodes, mut i: usize, node: usize, size: usize) {
        let mut child = i * 2 + 1;
        while child < size {
            if child + 1 < size && nodes.total[self.heap[child]] > nodes.total[self.heap[child + 1]]
            {
                child += 1;
            }
            self.heap[i] = self.heap[child];
            i = child;
            child = i * 2 + 1;
        }
        self.bubble_up(nodes, i, node);
    }

    /// `bubbleUp(m_size++, node)`
    fn push(&mut self, nodes: &NativeNodes, node: usize) {
        let i = self.heap.len();
        self.heap.push(node);
        self.bubble_up(nodes, i, node);
    }

    /// `top = m_heap[0]; --m_size; trickleDown(0, m_heap[m_size])`
    fn pop(&mut self, nodes: &NativeNodes) -> Option<usize> {
        if self.heap.is_empty() {
            return None;
        }
        let top = self.heap[0];
        let size = self.heap.len() - 1;
        let last = self.heap[size];
        self.heap.truncate(size);
        if size == 0 {
            return Some(top);
        }
        self.heap.push(last);
        // the vacated slot `size` is outside the heap while it trickles down
        self.trickle_down(nodes, 0, last, size);
        self.heap.truncate(size);
        Some(top)
    }
}

/// libPZPathFind `AStar::search(ISearchNode&, IGoalNode&)`: Detour's node queue, the open and closed flags of the
/// node, a closed node reopened when its new `total` is smaller, no closed set, and the heap key `total` =
/// `(float)(floor((h + g) * 1000 + 0.5) / 1000)`, and `h` added once more to the `g` of a successor in the goal.
/// `max_steps` negative is unlimited (the native `VGAStar` and `HLAStar` start with -1).
pub fn search_native<G: AStarNodes>(
    g: &mut G,
    initial: usize,
    max_steps: i32,
) -> Result<Option<usize>, String> {
    g.set_depth(initial, 0);
    let mut nodes = NativeNodes::default();
    nodes.ensure(initial);
    let mut open = NativeQueue::default();
    open.push(&nodes, initial);
    nodes.flags[initial] = (nodes.flags[initial] & 0xf8) + 1;
    let mut steps = 0;
    let mut successors: Vec<usize> = Vec::new();
    while !open.heap.is_empty() {
        if (0..=steps).contains(&max_steps) {
            break;
        }
        let Some(current) = open.pop(&nodes) else {
            break;
        };
        nodes.flags[current] = (nodes.flags[current] & 0xfc) | 2;
        if g.in_goal(current) {
            return Ok(Some(current));
        }
        successors.clear();
        g.get_successors(current, &mut successors)?;
        for &successor in &successors {
            if Some(successor) == g.parent(current) {
                continue;
            }
            nodes.ensure(successor);
            let old_flags = nodes.flags[successor];
            let mut new_g = g.g(current) + g.c(current, successor);
            let h = g.h(successor);
            if g.in_goal(successor) {
                new_g += g.h(successor);
            }
            let f = (((h + new_g) * 1000.0 + 0.5).floor() / 1000.0) as f32;
            // `(!open || f < total) && (!closed || f < total)`
            if ((old_flags & 1) == 0 && (nodes.flags[successor] & 2) == 0)
                || f < nodes.total[successor]
            {
                g.set_parent(successor, Some(current));
                let d = g.depth(current) + 1;
                g.set_depth(successor, d);
                nodes.flags[successor] &= 0xfd;
                g.set_g(successor, new_g);
                nodes.total[successor] = f;
                if (old_flags & 1) == 0 {
                    nodes.flags[successor] |= 1;
                    open.push(&nodes, successor);
                } else if let Some(i) = open.heap.iter().position(|&x| x == successor) {
                    open.bubble_up(&nodes, i, successor);
                }
            }
        }
        steps += 1;
    }
    Ok(None)
}
