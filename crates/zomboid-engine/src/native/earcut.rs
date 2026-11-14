//! mapbox `earcut.hpp` v2.2.3 (<https://github.com/mapbox/earcut.hpp>), the ear-clipping triangulator the game's
//! native `libPZClipper` calls from `Clipper.triangulate`, ported with `uint32_t` indices over Clipper2 `Point64`
//! rings (each coordinate read as `double`).
//!
//! Original copyright and license (kept as the ISC license requires):
//!
//! ```text
//! ISC License
//!
//! Copyright (c) 2015, Mapbox
//!
//! Permission to use, copy, modify, and/or distribute this software for any purpose
//! with or without fee is hereby granted, provided that the above copyright notice
//! and this permission notice appear in all copies.
//!
//! THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES WITH
//! REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF MERCHANTABILITY AND
//! FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR ANY SPECIAL, DIRECT,
//! INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES WHATSOEVER RESULTING FROM LOSS
//! OF USE, DATA OR PROFITS, WHETHER IN AN ACTION OF CONTRACT, NEGLIGENCE OR OTHER
//! TORTIOUS ACTION, ARISING OUT OF OR IN CONNECTION WITH THE USE OR PERFORMANCE OF
//! THIS SOFTWARE.
//! ```
//!
//! The release is read from the arm64 build shipped with 42.21.0: its z-order code multiplies by 32767 inside
//! `zOrder` with `inv_size = 1 / size` (v2.2.4 moved the 32767 into `inv_size`), and `eliminateHole` returns the
//! filtered bridge when it replaced the outer node (v2.2.3; v2.2.2 returned nothing). The function list has none of
//! the v3 helpers. Nodes live in an arena (indices instead of pointers). The holes are ordered by `std::sort` in the
//! original (unstable); here the sort is stable, so holes with equal leftmost x may be bridged in another order
//! (UNKNOWN whether the native order differs).
use super::clipper2::core::Point64;

#[derive(Clone, Copy, Debug)]
struct Node {
    i: u32,
    x: f64,
    y: f64,
    prev: usize,
    next: usize,
    z: i32,
    prev_z: Option<usize>,
    next_z: Option<usize>,
    steiner: bool,
}

/// mapbox::detail::Earcut<uint32_t>
struct Earcut {
    indices: Vec<u32>,
    vertices: usize,
    hashing: bool,
    min_x: f64,
    min_y: f64,
    inv_size: f64,
    nodes: Vec<Node>,
}

/// mapbox::earcut<uint32_t>: the triangles of a polygon given as its outer ring followed by its holes, as indices
/// into the rings' points taken in order (three per triangle).
pub fn earcut(polygon: &[Vec<Point64>]) -> Vec<u32> {
    let mut e = Earcut {
        indices: Vec::new(),
        vertices: 0,
        hashing: false,
        min_x: 0.0,
        min_y: 0.0,
        inv_size: 0.0,
        nodes: Vec::new(),
    };
    e.run(polygon);
    e.indices
}

impl Earcut {
    /// Earcut::operator()
    fn run(&mut self, points: &[Vec<Point64>]) {
        self.indices.clear();
        self.vertices = 0;
        if points.is_empty() {
            return;
        }
        let mut threshold: i32 = 80;
        let mut len = 0usize;
        let mut i = 0;
        while threshold >= 0 && i < points.len() {
            threshold -= points[i].len() as i32;
            len += points[i].len();
            i += 1;
        }
        self.nodes.clear();
        self.nodes.reserve(len * 3 / 2);
        self.indices.reserve(len + points[0].len());

        let Some(mut outer_node) = self.linked_list(&points[0], true) else {
            return;
        };
        if self.nodes[outer_node].prev == self.nodes[outer_node].next {
            return;
        }
        if points.len() > 1 {
            outer_node = self.eliminate_holes(points, outer_node);
        }
        self.hashing = threshold < 0;
        if self.hashing {
            let mut p = self.nodes[outer_node].next;
            let (mut min_x, mut max_x) = (self.nodes[outer_node].x, self.nodes[outer_node].x);
            let (mut min_y, mut max_y) = (self.nodes[outer_node].y, self.nodes[outer_node].y);
            loop {
                let (x, y) = (self.nodes[p].x, self.nodes[p].y);
                min_x = cpp_min(min_x, x);
                min_y = cpp_min(min_y, y);
                max_x = cpp_max(max_x, x);
                max_y = cpp_max(max_y, y);
                p = self.nodes[p].next;
                if p == outer_node {
                    break;
                }
            }
            self.min_x = min_x;
            self.min_y = min_y;
            let size = cpp_max(max_x - min_x, max_y - min_y);
            self.inv_size = if size != 0.0 { 1.0 / size } else { 0.0 };
        }
        self.earcut_linked(Some(outer_node), 0);
        self.nodes.clear();
    }

    /// Earcut::linkedList: a circular list in the requested winding order.
    fn linked_list(&mut self, points: &[Point64], clockwise: bool) -> Option<usize> {
        let mut sum = 0.0f64;
        let len = points.len();
        let mut last: Option<usize> = None;
        let mut j = if len > 0 { len - 1 } else { 0 };
        for i in 0..len {
            let (p1, p2) = (points[i], points[j]);
            let (p20, p10, p11, p21) = (p2.x as f64, p1.x as f64, p1.y as f64, p2.y as f64);
            sum += (p20 - p10) * (p11 + p21);
            j = i;
        }
        if clockwise == (sum > 0.0) {
            for (i, &p) in points.iter().enumerate() {
                last = Some(self.insert_node(self.vertices + i, p, last));
            }
        } else {
            for i in (0..len).rev() {
                last = Some(self.insert_node(self.vertices + i, points[i], last));
            }
        }
        if let Some(l) = last {
            let n = self.nodes[l].next;
            if self.equals(l, n) {
                self.remove_node(l);
                last = Some(self.nodes[l].next);
            }
        }
        self.vertices += len;
        last
    }

    /// Earcut::filterPoints: drops duplicate and collinear points (not Steiner points).
    fn filter_points(&mut self, start: usize, end: Option<usize>) -> usize {
        let mut end = end.unwrap_or(start);
        let mut p = start;
        loop {
            let mut again = false;
            let (pp, pn) = (self.nodes[p].prev, self.nodes[p].next);
            if !self.nodes[p].steiner && (self.equals(p, pn) || self.area(pp, p, pn) == 0.0) {
                self.remove_node(p);
                p = self.nodes[p].prev;
                end = p;
                if p == self.nodes[p].next {
                    break;
                }
                again = true;
            } else {
                p = pn;
            }
            if !(again || p != end) {
                break;
            }
        }
        end
    }

    /// Earcut::earcutLinked: the main ear-slicing loop.
    fn earcut_linked(&mut self, ear: Option<usize>, pass: i32) {
        let Some(mut ear) = ear else {
            return;
        };
        if pass == 0 && self.hashing {
            self.index_curve(ear);
        }
        let mut stop = ear;
        while self.nodes[ear].prev != self.nodes[ear].next {
            let prev = self.nodes[ear].prev;
            let next = self.nodes[ear].next;
            let is_ear = if self.hashing {
                self.is_ear_hashed(ear)
            } else {
                self.is_ear(ear)
            };
            if is_ear {
                self.indices.push(self.nodes[prev].i);
                self.indices.push(self.nodes[ear].i);
                self.indices.push(self.nodes[next].i);
                self.remove_node(ear);
                // skipping the next vertex leads to fewer sliver triangles
                ear = self.nodes[next].next;
                stop = self.nodes[next].next;
                continue;
            }
            ear = next;
            if ear == stop {
                if pass == 0 {
                    let f = self.filter_points(ear, None);
                    self.earcut_linked(Some(f), 1);
                } else if pass == 1 {
                    let f = self.filter_points(ear, None);
                    let cured = self.cure_local_intersections(f);
                    self.earcut_linked(Some(cured), 2);
                } else if pass == 2 {
                    self.split_earcut(ear);
                }
                break;
            }
        }
    }

    /// Earcut::isEar
    fn is_ear(&self, ear: usize) -> bool {
        let a = self.nodes[ear].prev;
        let b = ear;
        let c = self.nodes[ear].next;
        if self.area(a, b, c) >= 0.0 {
            return false; // reflex
        }
        let (na, nb, nc) = (self.nodes[a], self.nodes[b], self.nodes[c]);
        let mut p = self.nodes[c].next;
        while p != a {
            let np = self.nodes[p];
            if point_in_triangle(na.x, na.y, nb.x, nb.y, nc.x, nc.y, np.x, np.y)
                && self.area(np.prev, p, np.next) >= 0.0
            {
                return false;
            }
            p = np.next;
        }
        true
    }

    /// Earcut::isEarHashed
    fn is_ear_hashed(&self, ear: usize) -> bool {
        let a = self.nodes[ear].prev;
        let b = ear;
        let c = self.nodes[ear].next;
        if self.area(a, b, c) >= 0.0 {
            return false; // reflex
        }
        let (na, nb, nc) = (self.nodes[a], self.nodes[b], self.nodes[c]);
        let min_tx = cpp_min(na.x, cpp_min(nb.x, nc.x));
        let min_ty = cpp_min(na.y, cpp_min(nb.y, nc.y));
        let max_tx = cpp_max(na.x, cpp_max(nb.x, nc.x));
        let max_ty = cpp_max(na.y, cpp_max(nb.y, nc.y));
        let min_z = self.z_order(min_tx, min_ty);
        let max_z = self.z_order(max_tx, max_ty);

        let mut p = self.nodes[ear].next_z;
        while let Some(pi) = p.filter(|&pi| self.nodes[pi].z <= max_z) {
            let np = self.nodes[pi];
            if pi != a
                && pi != c
                && point_in_triangle(na.x, na.y, nb.x, nb.y, nc.x, nc.y, np.x, np.y)
                && self.area(np.prev, pi, np.next) >= 0.0
            {
                return false;
            }
            p = np.next_z;
        }
        p = self.nodes[ear].prev_z;
        while let Some(pi) = p.filter(|&pi| self.nodes[pi].z >= min_z) {
            let np = self.nodes[pi];
            if pi != a
                && pi != c
                && point_in_triangle(na.x, na.y, nb.x, nb.y, nc.x, nc.y, np.x, np.y)
                && self.area(np.prev, pi, np.next) >= 0.0
            {
                return false;
            }
            p = np.prev_z;
        }
        true
    }

    /// Earcut::cureLocalIntersections
    fn cure_local_intersections(&mut self, mut start: usize) -> usize {
        let mut p = start;
        loop {
            let a = self.nodes[p].prev;
            let pn = self.nodes[p].next;
            let b = self.nodes[pn].next;
            if !self.equals(a, b)
                && self.intersects(a, p, pn, b)
                && self.locally_inside(a, b)
                && self.locally_inside(b, a)
            {
                self.indices.push(self.nodes[a].i);
                self.indices.push(self.nodes[p].i);
                self.indices.push(self.nodes[b].i);
                self.remove_node(p);
                let pn2 = self.nodes[p].next;
                self.remove_node(pn2);
                p = b;
                start = b;
            }
            p = self.nodes[p].next;
            if p == start {
                break;
            }
        }
        self.filter_points(p, None)
    }

    /// Earcut::splitEarcut
    fn split_earcut(&mut self, start: usize) {
        let mut a = start;
        loop {
            let mut b = self.nodes[self.nodes[a].next].next;
            while b != self.nodes[a].prev {
                if self.nodes[a].i != self.nodes[b].i && self.is_valid_diagonal(a, b) {
                    let c = self.split_polygon(a, b);
                    let an = self.nodes[a].next;
                    let a2 = self.filter_points(a, Some(an));
                    let cn = self.nodes[c].next;
                    let c2 = self.filter_points(c, Some(cn));
                    self.earcut_linked(Some(a2), 0);
                    self.earcut_linked(Some(c2), 0);
                    return;
                }
                b = self.nodes[b].next;
            }
            a = self.nodes[a].next;
            if a == start {
                break;
            }
        }
    }

    /// Earcut::eliminateHoles
    fn eliminate_holes(&mut self, points: &[Vec<Point64>], mut outer_node: usize) -> usize {
        let mut queue = Vec::new();
        for ring in &points[1..] {
            if let Some(list) = self.linked_list(ring, false) {
                if list == self.nodes[list].next {
                    self.nodes[list].steiner = true;
                }
                queue.push(self.get_leftmost(list));
            }
        }
        let nodes = &self.nodes;
        queue.sort_by(|&a, &b| {
            nodes[a]
                .x
                .partial_cmp(&nodes[b].x)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        for &h in &queue {
            outer_node = self.eliminate_hole(h, outer_node);
            let on = self.nodes[outer_node].next;
            outer_node = self.filter_points(outer_node, Some(on));
        }
        outer_node
    }

    /// Earcut::eliminateHole
    fn eliminate_hole(&mut self, hole: usize, outer_node: usize) -> usize {
        let Some(bridge) = self.find_hole_bridge(hole, outer_node) else {
            return outer_node;
        };
        let bridge_reverse = self.split_polygon(bridge, hole);
        let bn = self.nodes[bridge].next;
        let filtered_bridge = self.filter_points(bridge, Some(bn));
        let brn = self.nodes[bridge_reverse].next;
        self.filter_points(bridge_reverse, Some(brn));
        if outer_node == bridge {
            filtered_bridge
        } else {
            outer_node
        }
    }

    /// Earcut::findHoleBridge (David Eberly's algorithm)
    fn find_hole_bridge(&self, hole: usize, outer_node: usize) -> Option<usize> {
        let mut p = outer_node;
        let hx = self.nodes[hole].x;
        let hy = self.nodes[hole].y;
        let mut qx = f64::NEG_INFINITY;
        let mut m: Option<usize> = None;
        loop {
            let np = self.nodes[p];
            let nn = self.nodes[np.next];
            if hy <= np.y && hy >= nn.y && nn.y != np.y {
                let x = np.x + (hy - np.y) * (nn.x - np.x) / (nn.y - np.y);
                if x <= hx && x > qx {
                    qx = x;
                    if x == hx {
                        if hy == np.y {
                            return Some(p);
                        }
                        if hy == nn.y {
                            return Some(np.next);
                        }
                    }
                    m = Some(if np.x < nn.x { p } else { np.next });
                }
            }
            p = np.next;
            if p == outer_node {
                break;
            }
        }
        let mut m = m?;
        if hx == qx {
            return Some(m); // the hole touches the outer segment: its leftmost endpoint
        }
        let stop = m;
        let mut tan_min = f64::INFINITY;
        p = m;
        let mx = self.nodes[m].x;
        let my = self.nodes[m].y;
        loop {
            let np = self.nodes[p];
            if hx >= np.x
                && np.x >= mx
                && hx != np.x
                && point_in_triangle(
                    if hy < my { hx } else { qx },
                    hy,
                    mx,
                    my,
                    if hy < my { qx } else { hx },
                    hy,
                    np.x,
                    np.y,
                )
            {
                let tan_cur = (hy - np.y).abs() / (hx - np.x);
                if self.locally_inside(p, hole)
                    && (tan_cur < tan_min
                        || (tan_cur == tan_min
                            && (np.x > self.nodes[m].x || self.sector_contains_sector(m, p))))
                {
                    m = p;
                    tan_min = tan_cur;
                }
            }
            p = np.next;
            if p == stop {
                break;
            }
        }
        Some(m)
    }

    /// Earcut::sectorContainsSector
    fn sector_contains_sector(&self, m: usize, p: usize) -> bool {
        self.area(self.nodes[m].prev, m, self.nodes[p].prev) < 0.0
            && self.area(self.nodes[p].next, m, self.nodes[m].next) < 0.0
    }

    /// Earcut::indexCurve: z-order values and the z-sorted list.
    fn index_curve(&mut self, start: usize) {
        let mut p = start;
        loop {
            if self.nodes[p].z == 0 {
                self.nodes[p].z = self.z_order(self.nodes[p].x, self.nodes[p].y);
            }
            self.nodes[p].prev_z = Some(self.nodes[p].prev);
            self.nodes[p].next_z = Some(self.nodes[p].next);
            p = self.nodes[p].next;
            if p == start {
                break;
            }
        }
        if let Some(pz) = self.nodes[p].prev_z {
            self.nodes[pz].next_z = None;
        }
        self.nodes[p].prev_z = None;
        self.sort_linked(p);
    }

    /// Earcut::sortLinked (Simon Tatham's linked-list merge sort, stable on z)
    fn sort_linked(&mut self, list: usize) -> usize {
        let mut list = Some(list);
        let mut in_size = 1;
        loop {
            let mut p = list;
            list = None;
            let mut tail: Option<usize> = None;
            let mut num_merges = 0;
            while let Some(p0) = p {
                num_merges += 1;
                let mut q = Some(p0);
                let mut p_size = 0;
                for _ in 0..in_size {
                    p_size += 1;
                    q = q.and_then(|qi| self.nodes[qi].next_z);
                    if q.is_none() {
                        break;
                    }
                }
                let mut q_size = in_size;
                while p_size > 0 || (q_size > 0 && q.is_some()) {
                    let e;
                    if p_size == 0 {
                        e = q.unwrap_or(usize::MAX);
                        q = self.nodes[e].next_z;
                        q_size -= 1;
                    } else if q_size == 0 || q.is_none() {
                        e = p.unwrap_or(usize::MAX);
                        p = self.nodes[e].next_z;
                        p_size -= 1;
                    } else {
                        let (pi, qi) = (p.unwrap_or(usize::MAX), q.unwrap_or(usize::MAX));
                        if self.nodes[pi].z <= self.nodes[qi].z {
                            e = pi;
                            p = self.nodes[e].next_z;
                            p_size -= 1;
                        } else {
                            e = qi;
                            q = self.nodes[e].next_z;
                            q_size -= 1;
                        }
                    }
                    match tail {
                        Some(t) => self.nodes[t].next_z = Some(e),
                        None => list = Some(e),
                    }
                    self.nodes[e].prev_z = tail;
                    tail = Some(e);
                }
                p = q;
            }
            if let Some(t) = tail {
                self.nodes[t].next_z = None;
            }
            if num_merges <= 1 {
                return list.unwrap_or(usize::MAX);
            }
            in_size *= 2;
        }
    }

    /// Earcut::zOrder: the coordinates scaled to 15 bits and interleaved.
    fn z_order(&self, x_: f64, y_: f64) -> i32 {
        let mut x = (32767.0 * (x_ - self.min_x) * self.inv_size) as i32;
        let mut y = (32767.0 * (y_ - self.min_y) * self.inv_size) as i32;
        x = (x | (x << 8)) & 0x00FF_00FF;
        x = (x | (x << 4)) & 0x0F0F_0F0F;
        x = (x | (x << 2)) & 0x3333_3333;
        x = (x | (x << 1)) & 0x5555_5555;
        y = (y | (y << 8)) & 0x00FF_00FF;
        y = (y | (y << 4)) & 0x0F0F_0F0F;
        y = (y | (y << 2)) & 0x3333_3333;
        y = (y | (y << 1)) & 0x5555_5555;
        x | (y << 1)
    }

    /// Earcut::getLeftmost
    fn get_leftmost(&self, start: usize) -> usize {
        let mut p = start;
        let mut leftmost = start;
        loop {
            let (np, nl) = (self.nodes[p], self.nodes[leftmost]);
            if np.x < nl.x || (np.x == nl.x && np.y < nl.y) {
                leftmost = p;
            }
            p = np.next;
            if p == start {
                break;
            }
        }
        leftmost
    }

    /// Earcut::isValidDiagonal
    fn is_valid_diagonal(&self, a: usize, b: usize) -> bool {
        let (na, nb) = (self.nodes[a], self.nodes[b]);
        self.nodes[na.next].i != nb.i
            && self.nodes[na.prev].i != nb.i
            && !self.intersects_polygon(a, b)
            && ((self.locally_inside(a, b)
                && self.locally_inside(b, a)
                && self.middle_inside(a, b)
                && (self.area(na.prev, a, nb.prev) != 0.0 || self.area(a, nb.prev, b) != 0.0))
                || (self.equals(a, b)
                    && self.area(na.prev, a, na.next) > 0.0
                    && self.area(nb.prev, b, nb.next) > 0.0))
    }

    /// Earcut::area: twice the signed area of the triangle.
    fn area(&self, p: usize, q: usize, r: usize) -> f64 {
        let (p, q, r) = (&self.nodes[p], &self.nodes[q], &self.nodes[r]);
        (q.y - p.y) * (r.x - q.x) - (q.x - p.x) * (r.y - q.y)
    }

    /// Earcut::equals
    fn equals(&self, p1: usize, p2: usize) -> bool {
        self.nodes[p1].x == self.nodes[p2].x && self.nodes[p1].y == self.nodes[p2].y
    }

    /// Earcut::intersects
    fn intersects(&self, p1: usize, q1: usize, p2: usize, q2: usize) -> bool {
        let o1 = sign(self.area(p1, q1, p2));
        let o2 = sign(self.area(p1, q1, q2));
        let o3 = sign(self.area(p2, q2, p1));
        let o4 = sign(self.area(p2, q2, q1));
        if o1 != o2 && o3 != o4 {
            return true;
        }
        (o1 == 0 && self.on_segment(p1, p2, q1))
            || (o2 == 0 && self.on_segment(p1, q2, q1))
            || (o3 == 0 && self.on_segment(p2, p1, q2))
            || (o4 == 0 && self.on_segment(p2, q1, q2))
    }

    /// Earcut::onSegment
    fn on_segment(&self, p: usize, q: usize, r: usize) -> bool {
        let (p, q, r) = (&self.nodes[p], &self.nodes[q], &self.nodes[r]);
        q.x <= cpp_max(p.x, r.x)
            && q.x >= cpp_min(p.x, r.x)
            && q.y <= cpp_max(p.y, r.y)
            && q.y >= cpp_min(p.y, r.y)
    }

    /// Earcut::intersectsPolygon
    fn intersects_polygon(&self, a: usize, b: usize) -> bool {
        let (ai, bi) = (self.nodes[a].i, self.nodes[b].i);
        let mut p = a;
        loop {
            let np = self.nodes[p];
            let ni = self.nodes[np.next].i;
            if np.i != ai && ni != ai && np.i != bi && ni != bi && self.intersects(p, np.next, a, b)
            {
                return true;
            }
            p = np.next;
            if p == a {
                break;
            }
        }
        false
    }

    /// Earcut::locallyInside
    fn locally_inside(&self, a: usize, b: usize) -> bool {
        let na = self.nodes[a];
        if self.area(na.prev, a, na.next) < 0.0 {
            self.area(a, b, na.next) >= 0.0 && self.area(a, na.prev, b) >= 0.0
        } else {
            self.area(a, b, na.prev) < 0.0 || self.area(a, na.next, b) < 0.0
        }
    }

    /// Earcut::middleInside
    fn middle_inside(&self, a: usize, b: usize) -> bool {
        let mut p = a;
        let mut inside = false;
        let px = (self.nodes[a].x + self.nodes[b].x) / 2.0;
        let py = (self.nodes[a].y + self.nodes[b].y) / 2.0;
        loop {
            let np = self.nodes[p];
            let nn = self.nodes[np.next];
            if ((np.y > py) != (nn.y > py))
                && nn.y != np.y
                && (px < (nn.x - np.x) * (py - np.y) / (nn.y - np.y) + np.x)
            {
                inside = !inside;
            }
            p = np.next;
            if p == a {
                break;
            }
        }
        inside
    }

    /// Earcut::splitPolygon: links two vertices by a bridge (splits a ring, or merges a hole into the outer ring).
    fn split_polygon(&mut self, a: usize, b: usize) -> usize {
        let a2 = self.new_node(self.nodes[a].i, self.nodes[a].x, self.nodes[a].y);
        let b2 = self.new_node(self.nodes[b].i, self.nodes[b].x, self.nodes[b].y);
        let an = self.nodes[a].next;
        let bp = self.nodes[b].prev;
        self.nodes[a].next = b;
        self.nodes[b].prev = a;
        self.nodes[a2].next = an;
        self.nodes[an].prev = a2;
        self.nodes[b2].next = a2;
        self.nodes[a2].prev = b2;
        self.nodes[bp].next = b2;
        self.nodes[b2].prev = bp;
        b2
    }

    fn new_node(&mut self, i: u32, x: f64, y: f64) -> usize {
        let n = self.nodes.len();
        self.nodes.push(Node {
            i,
            x,
            y,
            prev: n,
            next: n,
            z: 0,
            prev_z: None,
            next_z: None,
            steiner: false,
        });
        n
    }

    /// Earcut::insertNode
    fn insert_node(&mut self, i: usize, pt: Point64, last: Option<usize>) -> usize {
        let p = self.new_node(i as u32, pt.x as f64, pt.y as f64);
        if let Some(l) = last {
            let ln = self.nodes[l].next;
            self.nodes[p].next = ln;
            self.nodes[p].prev = l;
            self.nodes[ln].prev = p;
            self.nodes[l].next = p;
        }
        p
    }

    /// Earcut::removeNode
    fn remove_node(&mut self, p: usize) {
        let np = self.nodes[p];
        self.nodes[np.next].prev = np.prev;
        self.nodes[np.prev].next = np.next;
        if let Some(pz) = np.prev_z {
            self.nodes[pz].next_z = np.next_z;
        }
        if let Some(nz) = np.next_z {
            self.nodes[nz].prev_z = np.prev_z;
        }
    }
}

/// Earcut::pointInTriangle
#[allow(clippy::too_many_arguments)]
fn point_in_triangle(
    ax: f64,
    ay: f64,
    bx: f64,
    by: f64,
    cx: f64,
    cy: f64,
    px: f64,
    py: f64,
) -> bool {
    (cx - px) * (ay - py) - (ax - px) * (cy - py) >= 0.0
        && (ax - px) * (by - py) - (bx - px) * (ay - py) >= 0.0
        && (bx - px) * (cy - py) - (cx - px) * (by - py) >= 0.0
}

/// Earcut::sign
fn sign(val: f64) -> i32 {
    i32::from(0.0 < val) - i32::from(val < 0.0)
}

/// `std::min<double>(a, b)`: `b < a ? b : a`.
fn cpp_min(a: f64, b: f64) -> f64 {
    if b < a {
        b
    } else {
        a
    }
}

/// `std::max<double>(a, b)`: `a < b ? b : a`.
fn cpp_max(a: f64, b: f64) -> f64 {
    if a < b {
        b
    } else {
        a
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ring(pts: &[(i64, i64)]) -> Vec<Point64> {
        pts.iter().map(|&(x, y)| Point64::new(x, y)).collect()
    }

    fn twice_area(ring: &[Point64], idx: &[u32]) -> i64 {
        idx.chunks(3)
            .map(|t| {
                let (a, b, c) = (
                    ring[t[0] as usize],
                    ring[t[1] as usize],
                    ring[t[2] as usize],
                );
                ((b.x - a.x) * (c.y - a.y) - (c.x - a.x) * (b.y - a.y)).abs()
            })
            .sum()
    }

    /// A square becomes two triangles covering its area; an L of six corners becomes four.
    #[test]
    fn simple_rings_are_covered_exactly() {
        let sq = ring(&[(0, 0), (10, 0), (10, 10), (0, 10)]);
        let t = earcut(std::slice::from_ref(&sq));
        assert_eq!(t.len(), 6);
        assert_eq!(twice_area(&sq, &t), 200);
        let l = ring(&[(0, 0), (20, 0), (20, 10), (10, 10), (10, 20), (0, 20)]);
        let t = earcut(std::slice::from_ref(&l));
        assert_eq!(t.len(), 12);
        assert_eq!(twice_area(&l, &t), 2 * 300);
    }

    /// A square with a square hole: 8 triangles whose area is the ring's, 30x30 minus 10x10.
    #[test]
    fn a_hole_is_bridged_and_left_out() {
        let outer = ring(&[(0, 0), (30, 0), (30, 30), (0, 30)]);
        let hole = ring(&[(10, 10), (20, 10), (20, 20), (10, 20)]);
        let t = earcut(&[outer.clone(), hole.clone()]);
        let all: Vec<Point64> = outer.iter().chain(hole.iter()).copied().collect();
        assert_eq!(t.len(), 24);
        assert_eq!(twice_area(&all, &t), 2 * (900 - 100));
    }

    /// A ring of more than 80 points goes through the z-order (hashed) path and still covers its area: a 100x1
    /// staircase-free strip with 41 points on each long side.
    #[test]
    fn large_rings_use_the_hashed_path() {
        let mut pts: Vec<(i64, i64)> = (0..=40).map(|i| (i * 10, 0)).collect();
        pts.extend((0..=40).rev().map(|i| (i * 10, 7)));
        let r = ring(&pts);
        let t = earcut(std::slice::from_ref(&r));
        assert_eq!(twice_area(&r, &t), 2 * 400 * 7);
    }
}
