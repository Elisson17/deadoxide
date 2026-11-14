//! Clipper2Lib `clipper.engine.cpp` / `clipper.engine.h` (Clipper2 1.5.4, Angus Johnson, Boost Software License 1.0;
//! see `mod.rs`): the Vatti sweep of `Clipper64` (closed and open paths, every clip type and fill rule, flat paths or
//! a `PolyTree64`), as the game's native `libPZClipper` runs it. Pointers become indices into arenas owned by the
//! clipper (`delete` only unlinks); the algorithm, the order of operations and every tie rule are the original's.
//! `std::sort` of the intersection list (unstable in libc++) is a stable sort here: nodes at the same point may be
//! processed in another order (UNKNOWN whether it ever changes a result).
use std::collections::BinaryHeap;

use super::core::{
    cross_product_sign, dot_product, get_bounds, get_closest_point_on_segment,
    get_segment_intersect_pt, is_collinear, path2_contains_path1, perpendic_dist_from_line_sqrd,
    segments_intersect, Path64, Paths64, Point64, PointInPolygonResult, Rect64,
};

const NIL: usize = usize::MAX;

/// Clipper2Lib::ClipType
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClipType {
    NoClip,
    Intersection,
    Union,
    Difference,
    Xor,
}

/// Clipper2Lib::FillRule
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FillRule {
    EvenOdd,
    NonZero,
    Positive,
    Negative,
}

/// Clipper2Lib::PathType
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathType {
    Subject,
    Clip,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum JoinWith {
    NoJoin,
    Left,
    Right,
}

// Clipper2Lib::VertexFlags
const VF_OPEN_START: u32 = 1;
const VF_OPEN_END: u32 = 2;
const VF_LOCAL_MAX: u32 = 4;
const VF_LOCAL_MIN: u32 = 8;

#[derive(Clone, Copy, Debug)]
struct Vertex {
    pt: Point64,
    next: usize,
    prev: usize,
    flags: u32,
}

#[derive(Clone, Copy, Debug)]
struct LocalMinima {
    vertex: usize,
    polytype: PathType,
    is_open: bool,
}

#[derive(Clone, Debug)]
struct Active {
    bot: Point64,
    top: Point64,
    curr_x: i64,
    dx: f64,
    wind_dx: i32,
    wind_cnt: i32,
    wind_cnt2: i32,
    outrec: Option<usize>,
    prev_in_ael: Option<usize>,
    next_in_ael: Option<usize>,
    prev_in_sel: Option<usize>,
    next_in_sel: Option<usize>,
    jump: Option<usize>,
    vertex_top: usize,
    local_min: usize,
    is_left_bound: bool,
    join_with: JoinWith,
}

impl Default for Active {
    fn default() -> Self {
        Active {
            bot: Point64::default(),
            top: Point64::default(),
            curr_x: 0,
            dx: 0.0,
            wind_dx: 1,
            wind_cnt: 0,
            wind_cnt2: 0,
            outrec: None,
            prev_in_ael: None,
            next_in_ael: None,
            prev_in_sel: None,
            next_in_sel: None,
            jump: None,
            vertex_top: NIL,
            local_min: NIL,
            is_left_bound: false,
            join_with: JoinWith::NoJoin,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct OutPt {
    pt: Point64,
    next: usize,
    prev: usize,
    outrec: usize,
    /// `OutPt::horz` is only ever tested for null: whether a horizontal segment was registered at this point.
    horz: bool,
}

#[derive(Clone, Debug, Default)]
struct OutRec {
    idx: usize,
    owner: Option<usize>,
    front_edge: Option<usize>,
    back_edge: Option<usize>,
    pts: Option<usize>,
    polypath: Option<usize>,
    splits: Option<Vec<usize>>,
    recursive_split: Option<usize>,
    bounds: Rect64,
    path: Path64,
    is_open: bool,
}

#[derive(Clone, Copy, Debug)]
struct IntersectNode {
    pt: Point64,
    edge1: usize,
    edge2: usize,
}

#[derive(Clone, Copy, Debug)]
struct HorzSegment {
    left_op: usize,
    right_op: Option<usize>,
    left_to_right: bool,
}

#[derive(Clone, Copy, Debug)]
struct HorzJoin {
    op1: usize,
    op2: usize,
}

/// Clipper2Lib::PolyPath64 as a tree in one arena: node 0 is the root (the `PolyTree64` itself).
#[derive(Clone, Debug)]
pub struct PolyTree64 {
    pub nodes: Vec<PolyNode64>,
}

#[derive(Clone, Debug, Default)]
pub struct PolyNode64 {
    pub parent: Option<usize>,
    pub children: Vec<usize>,
    pub polygon: Path64,
}

impl Default for PolyTree64 {
    fn default() -> Self {
        PolyTree64 {
            nodes: vec![PolyNode64::default()],
        }
    }
}

impl PolyTree64 {
    /// Clipper2Lib::PolyPath64::Clear (of the root)
    pub fn clear(&mut self) {
        self.nodes.truncate(1);
        self.nodes[0].children.clear();
    }
    /// Clipper2Lib::PolyPath64::AddChild
    fn add_child(&mut self, parent: usize, path: &[Point64]) -> usize {
        let i = self.nodes.len();
        self.nodes.push(PolyNode64 {
            parent: Some(parent),
            children: Vec::new(),
            polygon: path.to_vec(),
        });
        self.nodes[parent].children.push(i);
        i
    }
    /// The children of a node, in insertion order (`PolyPath64::Child(i)`).
    pub fn children(&self, node: usize) -> &[usize] {
        &self.nodes[node].children
    }
}

/// Clipper2Lib::Clipper64 (with ClipperBase).
#[derive(Debug)]
pub struct Clipper64 {
    cliptype: ClipType,
    fillrule: FillRule,
    bot_y: i64,
    minima_list_sorted: bool,
    using_polytree: bool,
    actives: Option<usize>,
    sel: Option<usize>,
    minima_list: Vec<LocalMinima>,
    current_locmin: usize,
    vertices: Vec<Vertex>,
    scanline_list: BinaryHeap<i64>,
    intersect_nodes: Vec<IntersectNode>,
    horz_seg_list: Vec<HorzSegment>,
    horz_join_list: Vec<HorzJoin>,
    preserve_collinear: bool,
    reverse_solution: bool,
    has_open_paths: bool,
    succeeded: bool,
    outrec_list: Vec<OutRec>,
    ae: Vec<Active>,
    op: Vec<OutPt>,
}

impl Default for Clipper64 {
    fn default() -> Self {
        Self::new()
    }
}

impl Clipper64 {
    pub fn new() -> Self {
        Clipper64 {
            cliptype: ClipType::NoClip,
            fillrule: FillRule::EvenOdd,
            bot_y: 0,
            minima_list_sorted: false,
            using_polytree: false,
            actives: None,
            sel: None,
            minima_list: Vec::new(),
            current_locmin: 0,
            vertices: Vec::new(),
            scanline_list: BinaryHeap::new(),
            intersect_nodes: Vec::new(),
            horz_seg_list: Vec::new(),
            horz_join_list: Vec::new(),
            preserve_collinear: true,
            reverse_solution: false,
            has_open_paths: false,
            succeeded: true,
            outrec_list: Vec::new(),
            ae: Vec::new(),
            op: Vec::new(),
        }
    }

    /// ClipperBase::PreserveCollinear(bool)
    pub fn set_preserve_collinear(&mut self, v: bool) {
        self.preserve_collinear = v;
    }
    /// ClipperBase::ReverseSolution(bool)
    pub fn set_reverse_solution(&mut self, v: bool) {
        self.reverse_solution = v;
    }
    /// Clipper64::AddSubject
    pub fn add_subject(&mut self, paths: &[Path64]) {
        self.add_paths(paths, PathType::Subject, false);
    }
    /// Clipper64::AddOpenSubject
    pub fn add_open_subject(&mut self, paths: &[Path64]) {
        self.add_paths(paths, PathType::Subject, true);
    }
    /// Clipper64::AddClip
    pub fn add_clip(&mut self, paths: &[Path64]) {
        self.add_paths(paths, PathType::Clip, false);
    }

    // ---- small helpers (inline functions of clipper.engine.cpp) ----

    fn is_hot_edge(&self, e: usize) -> bool {
        self.ae[e].outrec.is_some()
    }
    fn is_open(&self, e: usize) -> bool {
        self.minima_list[self.ae[e].local_min].is_open
    }
    fn is_open_end_v(&self, v: usize) -> bool {
        self.vertices[v].flags & (VF_OPEN_START | VF_OPEN_END) != 0
    }
    fn is_open_end(&self, e: usize) -> bool {
        self.is_open_end_v(self.ae[e].vertex_top)
    }
    fn get_prev_hot_edge(&self, e: usize) -> Option<usize> {
        let mut prev = self.ae[e].prev_in_ael;
        while let Some(p) = prev {
            if self.is_open(p) || !self.is_hot_edge(p) {
                prev = self.ae[p].prev_in_ael;
            } else {
                break;
            }
        }
        prev
    }
    fn outrec_of(&self, e: usize) -> usize {
        self.ae[e].outrec.unwrap_or(NIL)
    }
    fn is_front(&self, e: usize) -> bool {
        self.outrec_list[self.outrec_of(e)].front_edge == Some(e)
    }
    fn top_x(&self, e: usize, current_y: i64) -> i64 {
        let a = &self.ae[e];
        if current_y == a.top.y || a.top.x == a.bot.x {
            a.top.x
        } else if current_y == a.bot.y {
            a.bot.x
        } else {
            a.bot.x + (a.dx * (current_y - a.bot.y) as f64).round_ties_even() as i64
        }
    }
    fn is_horizontal(&self, e: usize) -> bool {
        self.ae[e].top.y == self.ae[e].bot.y
    }
    fn is_heading_right_horz(&self, e: usize) -> bool {
        self.ae[e].dx == -f64::MAX
    }
    fn is_heading_left_horz(&self, e: usize) -> bool {
        self.ae[e].dx == f64::MAX
    }
    fn get_poly_type(&self, e: usize) -> PathType {
        self.minima_list[self.ae[e].local_min].polytype
    }
    fn is_same_poly_type(&self, e1: usize, e2: usize) -> bool {
        self.get_poly_type(e1) == self.get_poly_type(e2)
    }
    fn set_dx(&mut self, e: usize) {
        let (b, t) = (self.ae[e].bot, self.ae[e].top);
        self.ae[e].dx = get_dx(b, t);
    }
    fn next_vertex(&self, e: usize) -> usize {
        let v = self.ae[e].vertex_top;
        if self.ae[e].wind_dx > 0 {
            self.vertices[v].next
        } else {
            self.vertices[v].prev
        }
    }
    fn prev_prev_vertex(&self, e: usize) -> usize {
        let v = self.ae[e].vertex_top;
        if self.ae[e].wind_dx > 0 {
            self.vertices[self.vertices[v].prev].prev
        } else {
            self.vertices[self.vertices[v].next].next
        }
    }
    fn extract_from_sel(&mut self, ae: usize) -> Option<usize> {
        let res = self.ae[ae].next_in_sel;
        if let Some(r) = res {
            self.ae[r].prev_in_sel = self.ae[ae].prev_in_sel;
        }
        if let Some(p) = self.ae[ae].prev_in_sel {
            self.ae[p].next_in_sel = res;
        }
        res
    }
    fn insert1_before2_in_sel(&mut self, ae1: usize, ae2: usize) {
        self.ae[ae1].prev_in_sel = self.ae[ae2].prev_in_sel;
        if let Some(p) = self.ae[ae1].prev_in_sel {
            self.ae[p].next_in_sel = Some(ae1);
        }
        self.ae[ae1].next_in_sel = Some(ae2);
        self.ae[ae2].prev_in_sel = Some(ae1);
    }
    fn is_maxima_v(&self, v: usize) -> bool {
        self.vertices[v].flags & VF_LOCAL_MAX != 0
    }
    fn is_maxima(&self, e: usize) -> bool {
        self.is_maxima_v(self.ae[e].vertex_top)
    }
    fn get_curr_y_maxima_vertex_open(&self, e: usize) -> Option<usize> {
        let mut result = self.ae[e].vertex_top;
        if self.ae[e].wind_dx > 0 {
            while self.vertices[self.vertices[result].next].pt.y == self.vertices[result].pt.y
                && self.vertices[result].flags & (VF_OPEN_END | VF_LOCAL_MAX) == 0
            {
                result = self.vertices[result].next;
            }
        } else {
            while self.vertices[self.vertices[result].prev].pt.y == self.vertices[result].pt.y
                && self.vertices[result].flags & (VF_OPEN_END | VF_LOCAL_MAX) == 0
            {
                result = self.vertices[result].prev;
            }
        }
        self.is_maxima_v(result).then_some(result)
    }
    fn get_curr_y_maxima_vertex(&self, e: usize) -> Option<usize> {
        let mut result = self.ae[e].vertex_top;
        if self.ae[e].wind_dx > 0 {
            while self.vertices[self.vertices[result].next].pt.y == self.vertices[result].pt.y {
                result = self.vertices[result].next;
            }
        } else {
            while self.vertices[self.vertices[result].prev].pt.y == self.vertices[result].pt.y {
                result = self.vertices[result].prev;
            }
        }
        self.is_maxima_v(result).then_some(result)
    }
    fn get_maxima_pair(&self, e: usize) -> Option<usize> {
        let mut e2 = self.ae[e].next_in_ael;
        while let Some(x) = e2 {
            if self.ae[x].vertex_top == self.ae[e].vertex_top {
                return Some(x);
            }
            e2 = self.ae[x].next_in_ael;
        }
        None
    }
    fn new_out_pt(&mut self, pt: Point64, outrec: usize) -> usize {
        let i = self.op.len();
        self.op.push(OutPt {
            pt,
            next: i,
            prev: i,
            outrec,
            horz: false,
        });
        i
    }
    fn duplicate_op(&mut self, op: usize, insert_after: bool) -> usize {
        let (pt, outrec) = (self.op[op].pt, self.op[op].outrec);
        let result = self.new_out_pt(pt, outrec);
        if insert_after {
            self.op[result].next = self.op[op].next;
            let n = self.op[result].next;
            self.op[n].prev = result;
            self.op[result].prev = op;
            self.op[op].next = result;
        } else {
            self.op[result].prev = self.op[op].prev;
            let p = self.op[result].prev;
            self.op[p].next = result;
            self.op[result].next = op;
            self.op[op].prev = result;
        }
        result
    }
    fn dispose_out_pt(&mut self, op: usize) -> usize {
        let result = self.op[op].next;
        let (p, n) = (self.op[op].prev, self.op[op].next);
        self.op[p].next = n;
        self.op[n].prev = p;
        result
    }
    fn dispose_out_pts(&mut self, outrec: usize) {
        self.outrec_list[outrec].pts = None;
    }
    fn set_sides(&mut self, outrec: usize, start_edge: usize, end_edge: usize) {
        self.outrec_list[outrec].front_edge = Some(start_edge);
        self.outrec_list[outrec].back_edge = Some(end_edge);
    }
    fn swap_outrecs(&mut self, e1: usize, e2: usize) {
        let or1 = self.ae[e1].outrec;
        let or2 = self.ae[e2].outrec;
        if or1 == or2 {
            if let Some(o) = or1 {
                let r = &mut self.outrec_list[o];
                std::mem::swap(&mut r.front_edge, &mut r.back_edge);
            }
            return;
        }
        if let Some(o) = or1 {
            if self.outrec_list[o].front_edge == Some(e1) {
                self.outrec_list[o].front_edge = Some(e2);
            } else {
                self.outrec_list[o].back_edge = Some(e2);
            }
        }
        if let Some(o) = or2 {
            if self.outrec_list[o].front_edge == Some(e2) {
                self.outrec_list[o].front_edge = Some(e1);
            } else {
                self.outrec_list[o].back_edge = Some(e1);
            }
        }
        self.ae[e1].outrec = or2;
        self.ae[e2].outrec = or1;
    }
    /// Clipper2Lib::Area (OutPt): the shoelace sum in double.
    fn area_op(&self, op: usize) -> f64 {
        let mut result = 0.0;
        let mut op2 = op;
        loop {
            let p = self.op[self.op[op2].prev].pt;
            let c = self.op[op2].pt;
            result += (p.y + c.y) as f64 * (p.x - c.x) as f64;
            op2 = self.op[op2].next;
            if op2 == op {
                break;
            }
        }
        result * 0.5
    }
    fn get_real_outrec(&self, mut outrec: Option<usize>) -> Option<usize> {
        while let Some(o) = outrec {
            if self.outrec_list[o].pts.is_some() {
                break;
            }
            outrec = self.outrec_list[o].owner;
        }
        outrec
    }
    fn is_valid_owner(&self, outrec: usize, mut test_owner: Option<usize>) -> bool {
        while let Some(t) = test_owner {
            if t == outrec {
                break;
            }
            test_owner = self.outrec_list[t].owner;
        }
        test_owner.is_none()
    }
    fn uncouple_outrec(&mut self, ae: usize) {
        let Some(outrec) = self.ae[ae].outrec else {
            return;
        };
        if let Some(f) = self.outrec_list[outrec].front_edge {
            self.ae[f].outrec = None;
        }
        if let Some(b) = self.outrec_list[outrec].back_edge {
            self.ae[b].outrec = None;
        }
        self.outrec_list[outrec].front_edge = None;
        self.outrec_list[outrec].back_edge = None;
    }
    fn is_very_small_triangle(&self, op: usize) -> bool {
        let o = &self.op[op];
        let (next, prev) = (o.next, o.prev);
        self.op[next].next == prev
            && (pts_really_close(self.op[prev].pt, self.op[next].pt)
                || pts_really_close(o.pt, self.op[next].pt)
                || pts_really_close(o.pt, self.op[prev].pt))
    }
    fn is_valid_closed_path(&self, op: Option<usize>) -> bool {
        match op {
            Some(o) => {
                self.op[o].next != o
                    && self.op[o].next != self.op[o].prev
                    && !self.is_very_small_triangle(o)
            }
            None => false,
        }
    }
    fn outrec_is_ascending(&self, hot_edge: usize) -> bool {
        self.outrec_list[self.outrec_of(hot_edge)].front_edge == Some(hot_edge)
    }
    fn swap_front_back_sides(&mut self, outrec: usize) {
        let r = &mut self.outrec_list[outrec];
        std::mem::swap(&mut r.front_edge, &mut r.back_edge);
        if let Some(p) = r.pts {
            r.pts = Some(self.op[p].next);
        }
    }
    fn edges_adjacent_in_ael(&self, node: &IntersectNode) -> bool {
        self.ae[node.edge1].next_in_ael == Some(node.edge2)
            || self.ae[node.edge1].prev_in_ael == Some(node.edge2)
    }
    fn is_joined(&self, e: usize) -> bool {
        self.ae[e].join_with != JoinWith::NoJoin
    }
    fn set_owner(&mut self, outrec: usize, new_owner: usize) {
        self.outrec_list[new_owner].owner = self.get_real_outrec(self.outrec_list[new_owner].owner);
        let mut tmp = Some(new_owner);
        while let Some(t) = tmp {
            if t == outrec {
                break;
            }
            tmp = self.outrec_list[t].owner;
        }
        if tmp.is_some() {
            self.outrec_list[new_owner].owner = self.outrec_list[outrec].owner;
        }
        self.outrec_list[outrec].owner = Some(new_owner);
    }

    /// Clipper2Lib::PointInOpPolygon
    fn point_in_op_polygon(&self, pt: Point64, mut op: usize) -> PointInPolygonResult {
        use PointInPolygonResult::*;
        if op == self.op[op].next || self.op[op].prev == self.op[op].next {
            return IsOutside;
        }
        let mut op2 = op;
        loop {
            if self.op[op].pt.y != pt.y {
                break;
            }
            op = self.op[op].next;
            if op == op2 {
                break;
            }
        }
        if self.op[op].pt.y == pt.y {
            return IsOutside;
        }
        let mut is_above = self.op[op].pt.y < pt.y;
        let starting_above = is_above;
        let mut val = 0;
        op2 = self.op[op].next;
        while op2 != op {
            if is_above {
                while op2 != op && self.op[op2].pt.y < pt.y {
                    op2 = self.op[op2].next;
                }
            } else {
                while op2 != op && self.op[op2].pt.y > pt.y {
                    op2 = self.op[op2].next;
                }
            }
            if op2 == op {
                break;
            }
            let (c, p) = (self.op[op2].pt, self.op[self.op[op2].prev].pt);
            if c.y == pt.y {
                if c.x == pt.x || (c.y == p.y && ((pt.x < p.x) != (pt.x < c.x))) {
                    return IsOn;
                }
                op2 = self.op[op2].next;
                if op2 == op {
                    break;
                }
                continue;
            }
            if pt.x < c.x && pt.x < p.x {
                // only edges crossing on the left count
            } else if pt.x > p.x && pt.x > c.x {
                val = 1 - val;
            } else {
                let i = cross_product_sign(p, c, pt);
                if i == 0 {
                    return IsOn;
                }
                if (i < 0) == is_above {
                    val = 1 - val;
                }
            }
            is_above = !is_above;
            op2 = self.op[op2].next;
        }
        if is_above != starting_above {
            let (c, p) = (self.op[op2].pt, self.op[self.op[op2].prev].pt);
            let i = cross_product_sign(p, c, pt);
            if i == 0 {
                return IsOn;
            }
            if (i < 0) == is_above {
                val = 1 - val;
            }
        }
        if val == 0 {
            IsOutside
        } else {
            IsInside
        }
    }

    /// Clipper2Lib::GetCleanPath
    fn get_clean_path(&self, op: usize) -> Path64 {
        let mut result = Vec::new();
        let mut op2 = op;
        loop {
            let (c, n, p) = (
                self.op[op2].pt,
                self.op[self.op[op2].next].pt,
                self.op[self.op[op2].prev].pt,
            );
            if self.op[op2].next != op && ((c.x == n.x && c.x == p.x) || (c.y == n.y && c.y == p.y))
            {
                op2 = self.op[op2].next;
            } else {
                break;
            }
        }
        result.push(self.op[op2].pt);
        let mut prev_op = op2;
        op2 = self.op[op2].next;
        while op2 != op {
            let (c, n, p) = (
                self.op[op2].pt,
                self.op[self.op[op2].next].pt,
                self.op[prev_op].pt,
            );
            if (c.x != n.x || c.x != p.x) && (c.y != n.y || c.y != p.y) {
                result.push(c);
                prev_op = op2;
            }
            op2 = self.op[op2].next;
        }
        result
    }

    /// Clipper2Lib::Path2ContainsPath1 (OutPt)
    fn path2_contains_path1_op(&self, op1: usize, op2: usize) -> bool {
        use PointInPolygonResult::*;
        let mut pip = IsOn;
        let mut op = op1;
        loop {
            match self.point_in_op_polygon(self.op[op].pt, op2) {
                IsOutside => {
                    if pip == IsOutside {
                        return false;
                    }
                    pip = IsOutside;
                }
                IsInside => {
                    if pip == IsInside {
                        return true;
                    }
                    pip = IsInside;
                }
                IsOn => {}
            }
            op = self.op[op].next;
            if op == op1 {
                break;
            }
        }
        path2_contains_path1(&self.get_clean_path(op1), &self.get_clean_path(op2))
    }

    /// Clipper2Lib::AddPaths_
    fn add_paths_(&mut self, paths: &[Path64], polytype: PathType, is_open: bool) {
        let total_vertex_count: usize = paths.iter().map(Vec::len).sum();
        if total_vertex_count == 0 {
            return;
        }
        let base = self.vertices.len();
        self.vertices.resize(
            base + total_vertex_count,
            Vertex {
                pt: Point64::default(),
                next: NIL,
                prev: NIL,
                flags: 0,
            },
        );
        let mut v = base;
        for path in paths {
            let v0 = v;
            let mut curr_v = v;
            let mut prev_v: Option<usize> = None;
            if path.is_empty() {
                continue;
            }
            self.vertices[v].prev = NIL;
            let mut cnt = 0;
            for &pt in path {
                if let Some(p) = prev_v {
                    if self.vertices[p].pt == pt {
                        continue;
                    }
                    self.vertices[p].next = curr_v;
                }
                self.vertices[curr_v].prev = prev_v.unwrap_or(NIL);
                self.vertices[curr_v].pt = pt;
                self.vertices[curr_v].flags = 0;
                prev_v = Some(curr_v);
                curr_v += 1;
                cnt += 1;
            }
            let Some(mut pv) = prev_v else {
                continue;
            };
            if self.vertices[pv].prev == NIL {
                continue;
            }
            if !is_open && self.vertices[pv].pt == self.vertices[v0].pt {
                pv = self.vertices[pv].prev;
            }
            self.vertices[pv].next = v0;
            self.vertices[v0].prev = pv;
            v = curr_v;
            if cnt < 2 || (cnt == 2 && !is_open) {
                continue;
            }
            // find and assign the local minima
            let mut going_up;
            if is_open {
                let mut cv = self.vertices[v0].next;
                while cv != v0 && self.vertices[cv].pt.y == self.vertices[v0].pt.y {
                    cv = self.vertices[cv].next;
                }
                going_up = self.vertices[cv].pt.y <= self.vertices[v0].pt.y;
                if going_up {
                    self.vertices[v0].flags = VF_OPEN_START;
                    self.add_loc_min(v0, polytype, true);
                } else {
                    self.vertices[v0].flags = VF_OPEN_START | VF_LOCAL_MAX;
                }
            } else {
                let mut pv2 = self.vertices[v0].prev;
                while pv2 != v0 && self.vertices[pv2].pt.y == self.vertices[v0].pt.y {
                    pv2 = self.vertices[pv2].prev;
                }
                if pv2 == v0 {
                    continue; // only open paths can be completely flat
                }
                going_up = self.vertices[pv2].pt.y > self.vertices[v0].pt.y;
            }
            let going_up0 = going_up;
            let mut pv = v0;
            let mut cv = self.vertices[v0].next;
            while cv != v0 {
                if self.vertices[cv].pt.y > self.vertices[pv].pt.y && going_up {
                    self.vertices[pv].flags |= VF_LOCAL_MAX;
                    going_up = false;
                } else if self.vertices[cv].pt.y < self.vertices[pv].pt.y && !going_up {
                    going_up = true;
                    self.add_loc_min(pv, polytype, is_open);
                }
                pv = cv;
                cv = self.vertices[cv].next;
            }
            if is_open {
                self.vertices[pv].flags |= VF_OPEN_END;
                if going_up {
                    self.vertices[pv].flags |= VF_LOCAL_MAX;
                } else {
                    self.add_loc_min(pv, polytype, is_open);
                }
            } else if going_up != going_up0 {
                if going_up0 {
                    self.add_loc_min(pv, polytype, false);
                } else {
                    self.vertices[pv].flags |= VF_LOCAL_MAX;
                }
            }
        }
    }

    /// ClipperBase::AddLocMin
    fn add_loc_min(&mut self, vert: usize, polytype: PathType, is_open: bool) {
        if self.vertices[vert].flags & VF_LOCAL_MIN != 0 {
            return;
        }
        self.vertices[vert].flags |= VF_LOCAL_MIN;
        self.minima_list.push(LocalMinima {
            vertex: vert,
            polytype,
            is_open,
        });
    }

    /// ClipperBase::AddPaths
    pub fn add_paths(&mut self, paths: &[Path64], polytype: PathType, is_open: bool) {
        if is_open {
            self.has_open_paths = true;
        }
        self.minima_list_sorted = false;
        self.add_paths_(paths, polytype, is_open);
    }

    /// ClipperBase::CleanUp
    fn clean_up(&mut self) {
        self.actives = None;
        self.ae.clear();
        self.scanline_list.clear();
        self.intersect_nodes.clear();
        self.outrec_list.clear();
        self.op.clear();
        self.horz_seg_list.clear();
        self.horz_join_list.clear();
    }

    /// ClipperBase::Clear
    pub fn clear(&mut self) {
        self.clean_up();
        self.minima_list.clear();
        self.vertices.clear();
        self.current_locmin = 0;
        self.minima_list_sorted = false;
        self.has_open_paths = false;
    }

    /// ClipperBase::Reset: the local minima sorted (std::stable_sort, LocMinSorter: larger y first, then smaller x)
    /// and their levels queued as scanlines.
    fn reset(&mut self) {
        if !self.minima_list_sorted {
            let vertices = &self.vertices;
            self.minima_list.sort_by(|a, b| {
                let (pa, pb) = (vertices[a.vertex].pt, vertices[b.vertex].pt);
                pb.y.cmp(&pa.y).then(pa.x.cmp(&pb.x))
            });
            self.minima_list_sorted = true;
        }
        for i in (0..self.minima_list.len()).rev() {
            let y = self.vertices[self.minima_list[i].vertex].pt.y;
            self.scanline_list.push(y);
        }
        self.current_locmin = 0;
        self.actives = None;
        self.sel = None;
        self.succeeded = true;
    }

    fn insert_scanline(&mut self, y: i64) {
        self.scanline_list.push(y);
    }

    fn pop_scanline(&mut self) -> Option<i64> {
        let y = self.scanline_list.pop()?;
        while self.scanline_list.peek() == Some(&y) {
            self.scanline_list.pop();
        }
        Some(y)
    }

    fn pop_local_minima(&mut self, y: i64) -> Option<usize> {
        if self.current_locmin == self.minima_list.len()
            || self.vertices[self.minima_list[self.current_locmin].vertex]
                .pt
                .y
                != y
        {
            return None;
        }
        self.current_locmin += 1;
        Some(self.current_locmin - 1)
    }

    /// ClipperBase::IsContributingClosed
    fn is_contributing_closed(&self, e: usize) -> bool {
        let a = &self.ae[e];
        match self.fillrule {
            FillRule::EvenOdd => {}
            FillRule::NonZero => {
                if a.wind_cnt.abs() != 1 {
                    return false;
                }
            }
            FillRule::Positive => {
                if a.wind_cnt != 1 {
                    return false;
                }
            }
            FillRule::Negative => {
                if a.wind_cnt != -1 {
                    return false;
                }
            }
        }
        match self.cliptype {
            ClipType::NoClip => false,
            ClipType::Intersection => match self.fillrule {
                FillRule::Positive => a.wind_cnt2 > 0,
                FillRule::Negative => a.wind_cnt2 < 0,
                _ => a.wind_cnt2 != 0,
            },
            ClipType::Union => match self.fillrule {
                FillRule::Positive => a.wind_cnt2 <= 0,
                FillRule::Negative => a.wind_cnt2 >= 0,
                _ => a.wind_cnt2 == 0,
            },
            ClipType::Difference => {
                let result = match self.fillrule {
                    FillRule::Positive => a.wind_cnt2 <= 0,
                    FillRule::Negative => a.wind_cnt2 >= 0,
                    _ => a.wind_cnt2 == 0,
                };
                if self.get_poly_type(e) == PathType::Subject {
                    result
                } else {
                    !result
                }
            }
            ClipType::Xor => true,
        }
    }

    /// ClipperBase::IsContributingOpen
    fn is_contributing_open(&self, e: usize) -> bool {
        let a = &self.ae[e];
        let (is_in_clip, is_in_subj) = match self.fillrule {
            FillRule::Positive => (a.wind_cnt2 > 0, a.wind_cnt > 0),
            FillRule::Negative => (a.wind_cnt2 < 0, a.wind_cnt < 0),
            _ => (a.wind_cnt2 != 0, a.wind_cnt != 0),
        };
        match self.cliptype {
            ClipType::Intersection => is_in_clip,
            ClipType::Union => !is_in_subj && !is_in_clip,
            _ => !is_in_clip,
        }
    }

    /// ClipperBase::SetWindCountForClosedPathEdge
    fn set_wind_count_for_closed_path_edge(&mut self, e: usize) {
        let mut e2 = self.ae[e].prev_in_ael;
        let pt = self.get_poly_type(e);
        while let Some(x) = e2 {
            if self.get_poly_type(x) != pt || self.is_open(x) {
                e2 = self.ae[x].prev_in_ael;
            } else {
                break;
            }
        }
        let mut e2 = match e2 {
            None => {
                self.ae[e].wind_cnt = self.ae[e].wind_dx;
                self.actives
            }
            Some(x) if self.fillrule == FillRule::EvenOdd => {
                self.ae[e].wind_cnt = self.ae[e].wind_dx;
                self.ae[e].wind_cnt2 = self.ae[x].wind_cnt2;
                self.ae[x].next_in_ael
            }
            Some(x) => {
                let (x_wc, x_wd, e_wd) =
                    (self.ae[x].wind_cnt, self.ae[x].wind_dx, self.ae[e].wind_dx);
                if x_wc * x_wd < 0 {
                    if x_wc.abs() > 1 {
                        if x_wd * e_wd < 0 {
                            self.ae[e].wind_cnt = x_wc;
                        } else {
                            self.ae[e].wind_cnt = x_wc + e_wd;
                        }
                    } else {
                        self.ae[e].wind_cnt = if self.is_open(e) { 1 } else { e_wd };
                    }
                } else if x_wd * e_wd < 0 {
                    self.ae[e].wind_cnt = x_wc;
                } else {
                    self.ae[e].wind_cnt = x_wc + e_wd;
                }
                self.ae[e].wind_cnt2 = self.ae[x].wind_cnt2;
                self.ae[x].next_in_ael
            }
        };
        if self.fillrule == FillRule::EvenOdd {
            while let Some(x) = e2 {
                if x == e {
                    break;
                }
                if self.get_poly_type(x) != pt && !self.is_open(x) {
                    self.ae[e].wind_cnt2 = if self.ae[e].wind_cnt2 == 0 { 1 } else { 0 };
                }
                e2 = self.ae[x].next_in_ael;
            }
        } else {
            while let Some(x) = e2 {
                if x == e {
                    break;
                }
                if self.get_poly_type(x) != pt && !self.is_open(x) {
                    self.ae[e].wind_cnt2 += self.ae[x].wind_dx;
                }
                e2 = self.ae[x].next_in_ael;
            }
        }
    }

    /// ClipperBase::SetWindCountForOpenPathEdge
    fn set_wind_count_for_open_path_edge(&mut self, e: usize) {
        let mut e2 = self.actives;
        if self.fillrule == FillRule::EvenOdd {
            let (mut cnt1, mut cnt2) = (0, 0);
            while let Some(x) = e2 {
                if x == e {
                    break;
                }
                if self.get_poly_type(x) == PathType::Clip {
                    cnt2 += 1;
                } else if !self.is_open(x) {
                    cnt1 += 1;
                }
                e2 = self.ae[x].next_in_ael;
            }
            self.ae[e].wind_cnt = cnt1 & 1;
            self.ae[e].wind_cnt2 = cnt2 & 1;
        } else {
            while let Some(x) = e2 {
                if x == e {
                    break;
                }
                if self.get_poly_type(x) == PathType::Clip {
                    self.ae[e].wind_cnt2 += self.ae[x].wind_dx;
                } else if !self.is_open(x) {
                    self.ae[e].wind_cnt += self.ae[x].wind_dx;
                }
                e2 = self.ae[x].next_in_ael;
            }
        }
    }

    /// Clipper2Lib::IsValidAelOrder
    fn is_valid_ael_order(&self, resident: usize, newcomer: usize) -> bool {
        let (r, n) = (&self.ae[resident], &self.ae[newcomer]);
        if n.curr_x != r.curr_x {
            return n.curr_x > r.curr_x;
        }
        let i = cross_product_sign(r.top, n.bot, n.top);
        if i != 0 {
            return i < 0;
        }
        if !self.is_maxima(resident) && r.top.y > n.top.y {
            return cross_product_sign(n.bot, r.top, self.vertices[self.next_vertex(resident)].pt)
                <= 0;
        } else if !self.is_maxima(newcomer) && n.top.y > r.top.y {
            return cross_product_sign(n.bot, n.top, self.vertices[self.next_vertex(newcomer)].pt)
                >= 0;
        }
        let y = n.bot.y;
        let newcomer_is_left = n.is_left_bound;
        if r.bot.y != y || self.vertices[self.minima_list[r.local_min].vertex].pt.y != y {
            n.is_left_bound
        } else if r.is_left_bound != newcomer_is_left {
            newcomer_is_left
        } else if is_collinear(
            self.vertices[self.prev_prev_vertex(resident)].pt,
            r.bot,
            r.top,
        ) {
            true
        } else {
            (cross_product_sign(
                self.vertices[self.prev_prev_vertex(resident)].pt,
                n.bot,
                self.vertices[self.prev_prev_vertex(newcomer)].pt,
            ) > 0)
                == newcomer_is_left
        }
    }

    /// ClipperBase::InsertLeftEdge
    fn insert_left_edge(&mut self, e: usize) {
        match self.actives {
            None => {
                self.ae[e].prev_in_ael = None;
                self.ae[e].next_in_ael = None;
                self.actives = Some(e);
            }
            Some(a) if !self.is_valid_ael_order(a, e) => {
                self.ae[e].prev_in_ael = None;
                self.ae[e].next_in_ael = Some(a);
                self.ae[a].prev_in_ael = Some(e);
                self.actives = Some(e);
            }
            Some(a) => {
                let mut e2 = a;
                while let Some(n) = self.ae[e2].next_in_ael {
                    if self.is_valid_ael_order(n, e) {
                        e2 = n;
                    } else {
                        break;
                    }
                }
                if self.ae[e2].join_with == JoinWith::Right {
                    match self.ae[e2].next_in_ael {
                        Some(n) => e2 = n,
                        None => return,
                    }
                }
                self.ae[e].next_in_ael = self.ae[e2].next_in_ael;
                if let Some(n) = self.ae[e2].next_in_ael {
                    self.ae[n].prev_in_ael = Some(e);
                }
                self.ae[e].prev_in_ael = Some(e2);
                self.ae[e2].next_in_ael = Some(e);
            }
        }
    }

    /// Clipper2Lib::InsertRightEdge
    fn insert_right_edge(&mut self, e: usize, e2: usize) {
        self.ae[e2].next_in_ael = self.ae[e].next_in_ael;
        if let Some(n) = self.ae[e].next_in_ael {
            self.ae[n].prev_in_ael = Some(e2);
        }
        self.ae[e2].prev_in_ael = Some(e);
        self.ae[e].next_in_ael = Some(e2);
    }

    fn new_active(&mut self, a: Active) -> usize {
        self.ae.push(a);
        self.ae.len() - 1
    }

    /// ClipperBase::InsertLocalMinimaIntoAEL
    fn insert_local_minima_into_ael(&mut self, bot_y: i64) {
        while let Some(lm) = self.pop_local_minima(bot_y) {
            let vertex = self.minima_list[lm].vertex;
            let vflags = self.vertices[vertex].flags;
            let mut left_bound = if vflags & VF_OPEN_START != 0 {
                None
            } else {
                let vt = self.vertices[vertex].prev;
                let bot = self.vertices[vertex].pt;
                let e = self.new_active(Active {
                    bot,
                    curr_x: bot.x,
                    wind_dx: -1,
                    vertex_top: vt,
                    top: self.vertices[vt].pt,
                    local_min: lm,
                    ..Active::default()
                });
                self.set_dx(e);
                Some(e)
            };
            let mut right_bound = if vflags & VF_OPEN_END != 0 {
                None
            } else {
                let vt = self.vertices[vertex].next;
                let bot = self.vertices[vertex].pt;
                let e = self.new_active(Active {
                    bot,
                    curr_x: bot.x,
                    wind_dx: 1,
                    vertex_top: vt,
                    top: self.vertices[vt].pt,
                    local_min: lm,
                    ..Active::default()
                });
                self.set_dx(e);
                Some(e)
            };
            if let (Some(l), Some(r)) = (left_bound, right_bound) {
                if self.is_horizontal(l) {
                    if self.is_heading_right_horz(l) {
                        std::mem::swap(&mut left_bound, &mut right_bound);
                    }
                } else if self.is_horizontal(r) {
                    if self.is_heading_left_horz(r) {
                        std::mem::swap(&mut left_bound, &mut right_bound);
                    }
                } else if self.ae[l].dx < self.ae[r].dx {
                    std::mem::swap(&mut left_bound, &mut right_bound);
                }
            } else if left_bound.is_none() {
                left_bound = right_bound;
                right_bound = None;
            }
            let Some(left_bound) = left_bound else {
                continue;
            };
            self.ae[left_bound].is_left_bound = true;
            self.insert_left_edge(left_bound);
            let contributing = if self.is_open(left_bound) {
                self.set_wind_count_for_open_path_edge(left_bound);
                self.is_contributing_open(left_bound)
            } else {
                self.set_wind_count_for_closed_path_edge(left_bound);
                self.is_contributing_closed(left_bound)
            };
            if let Some(right_bound) = right_bound {
                self.ae[right_bound].is_left_bound = false;
                self.ae[right_bound].wind_cnt = self.ae[left_bound].wind_cnt;
                self.ae[right_bound].wind_cnt2 = self.ae[left_bound].wind_cnt2;
                self.insert_right_edge(left_bound, right_bound);
                if contributing {
                    let bot = self.ae[left_bound].bot;
                    self.add_local_min_poly(left_bound, right_bound, bot, true);
                    if !self.is_horizontal(left_bound) {
                        self.check_join_left(left_bound, bot, false);
                    }
                }
                while let Some(n) = self.ae[right_bound].next_in_ael {
                    if !self.is_valid_ael_order(n, right_bound) {
                        break;
                    }
                    let bot = self.ae[right_bound].bot;
                    self.intersect_edges(right_bound, n, bot);
                    self.swap_positions_in_ael(right_bound, n);
                }
                if self.is_horizontal(right_bound) {
                    self.push_horz(right_bound);
                } else {
                    let bot = self.ae[right_bound].bot;
                    self.check_join_right(right_bound, bot, false);
                    let ty = self.ae[right_bound].top.y;
                    self.insert_scanline(ty);
                }
            } else if contributing {
                let bot = self.ae[left_bound].bot;
                self.start_open_path(left_bound, bot);
            }
            if self.is_horizontal(left_bound) {
                self.push_horz(left_bound);
            } else {
                let ty = self.ae[left_bound].top.y;
                self.insert_scanline(ty);
            }
        }
    }

    fn push_horz(&mut self, e: usize) {
        self.ae[e].next_in_sel = self.sel;
        self.sel = Some(e);
    }

    fn pop_horz(&mut self) -> Option<usize> {
        let e = self.sel?;
        self.sel = self.ae[e].next_in_sel;
        Some(e)
    }

    /// ClipperBase::AddLocalMinPoly
    fn add_local_min_poly(&mut self, e1: usize, e2: usize, pt: Point64, is_new: bool) -> usize {
        let outrec = self.new_outrec();
        self.ae[e1].outrec = Some(outrec);
        self.ae[e2].outrec = Some(outrec);
        if self.is_open(e1) {
            self.outrec_list[outrec].owner = None;
            self.outrec_list[outrec].is_open = true;
            if self.ae[e1].wind_dx > 0 {
                self.set_sides(outrec, e1, e2);
            } else {
                self.set_sides(outrec, e2, e1);
            }
        } else {
            let prev_hot_edge = self.get_prev_hot_edge(e1);
            if let Some(ph) = prev_hot_edge {
                if self.using_polytree {
                    let po = self.outrec_of(ph);
                    self.set_owner(outrec, po);
                }
                if self.outrec_is_ascending(ph) == is_new {
                    self.set_sides(outrec, e2, e1);
                } else {
                    self.set_sides(outrec, e1, e2);
                }
            } else {
                self.outrec_list[outrec].owner = None;
                if is_new {
                    self.set_sides(outrec, e1, e2);
                } else {
                    self.set_sides(outrec, e2, e1);
                }
            }
        }
        let op = self.new_out_pt(pt, outrec);
        self.outrec_list[outrec].pts = Some(op);
        op
    }

    /// ClipperBase::AddLocalMaxPoly
    fn add_local_max_poly(&mut self, e1: usize, e2: usize, pt: Point64) -> Option<usize> {
        if self.is_joined(e1) {
            self.split(e1, pt);
        }
        if self.is_joined(e2) {
            self.split(e2, pt);
        }
        if self.is_front(e1) == self.is_front(e2) {
            if self.is_open_end(e1) {
                let o = self.outrec_of(e1);
                self.swap_front_back_sides(o);
            } else if self.is_open_end(e2) {
                let o = self.outrec_of(e2);
                self.swap_front_back_sides(o);
            } else {
                self.succeeded = false;
                return None;
            }
        }
        let mut result = self.add_out_pt(e1, pt);
        if self.ae[e1].outrec == self.ae[e2].outrec {
            let outrec = self.outrec_of(e1);
            self.outrec_list[outrec].pts = Some(result);
            if self.using_polytree {
                match self.get_prev_hot_edge(e1) {
                    None => self.outrec_list[outrec].owner = None,
                    Some(e) => {
                        let eo = self.outrec_of(e);
                        self.set_owner(outrec, eo);
                    }
                }
            }
            self.uncouple_outrec(e1);
            result = self.outrec_list[outrec].pts.unwrap_or(result);
            if let Some(owner) = self.outrec_list[outrec].owner {
                if self.outrec_list[owner].front_edge.is_none() {
                    self.outrec_list[outrec].owner = self.get_real_outrec(Some(owner));
                }
            }
        } else if self.is_open(e1) {
            if self.ae[e1].wind_dx < 0 {
                self.join_outrec_paths(e1, e2);
            } else {
                self.join_outrec_paths(e2, e1);
            }
        } else if self.outrec_list[self.outrec_of(e1)].idx
            < self.outrec_list[self.outrec_of(e2)].idx
        {
            self.join_outrec_paths(e1, e2);
        } else {
            self.join_outrec_paths(e2, e1);
        }
        Some(result)
    }

    /// ClipperBase::JoinOutrecPaths
    fn join_outrec_paths(&mut self, e1: usize, e2: usize) {
        let (or1, or2) = (self.outrec_of(e1), self.outrec_of(e2));
        let p1_st = self.outrec_list[or1].pts.unwrap_or(NIL);
        let p2_st = self.outrec_list[or2].pts.unwrap_or(NIL);
        let p1_end = self.op[p1_st].next;
        let p2_end = self.op[p2_st].next;
        if self.is_front(e1) {
            self.op[p2_end].prev = p1_st;
            self.op[p1_st].next = p2_end;
            self.op[p2_st].next = p1_end;
            self.op[p1_end].prev = p2_st;
            self.outrec_list[or1].pts = Some(p2_st);
            self.outrec_list[or1].front_edge = self.outrec_list[or2].front_edge;
            if let Some(f) = self.outrec_list[or1].front_edge {
                self.ae[f].outrec = Some(or1);
            }
        } else {
            self.op[p1_end].prev = p2_st;
            self.op[p2_st].next = p1_end;
            self.op[p1_st].next = p2_end;
            self.op[p2_end].prev = p1_st;
            self.outrec_list[or1].back_edge = self.outrec_list[or2].back_edge;
            if let Some(b) = self.outrec_list[or1].back_edge {
                self.ae[b].outrec = Some(or1);
            }
        }
        self.outrec_list[or2].front_edge = None;
        self.outrec_list[or2].back_edge = None;
        self.outrec_list[or2].pts = None;
        if self.is_open_end(e1) {
            self.outrec_list[or2].pts = self.outrec_list[or1].pts;
            self.outrec_list[or1].pts = None;
        } else {
            self.set_owner(or2, or1);
        }
        self.ae[e1].outrec = None;
        self.ae[e2].outrec = None;
    }

    /// ClipperBase::NewOutRec
    fn new_outrec(&mut self) -> usize {
        let idx = self.outrec_list.len();
        self.outrec_list.push(OutRec {
            idx,
            ..OutRec::default()
        });
        idx
    }

    /// ClipperBase::AddOutPt
    fn add_out_pt(&mut self, e: usize, pt: Point64) -> usize {
        let outrec = self.outrec_of(e);
        let to_front = self.is_front(e);
        let op_front = self.outrec_list[outrec].pts.unwrap_or(NIL);
        let op_back = self.op[op_front].next;
        if to_front {
            if pt == self.op[op_front].pt {
                return op_front;
            }
        } else if pt == self.op[op_back].pt {
            return op_back;
        }
        let new_op = self.new_out_pt(pt, outrec);
        self.op[op_back].prev = new_op;
        self.op[new_op].prev = op_front;
        self.op[new_op].next = op_back;
        self.op[op_front].next = new_op;
        if to_front {
            self.outrec_list[outrec].pts = Some(new_op);
        }
        new_op
    }

    /// ClipperBase::CleanCollinear
    fn clean_collinear(&mut self, outrec: usize) {
        let Some(outrec) = self.get_real_outrec(Some(outrec)) else {
            return;
        };
        if self.outrec_list[outrec].is_open {
            return;
        }
        if !self.is_valid_closed_path(self.outrec_list[outrec].pts) {
            self.dispose_out_pts(outrec);
            return;
        }
        let Some(mut start_op) = self.outrec_list[outrec].pts else {
            return;
        };
        let mut op2 = start_op;
        loop {
            let (p, c, n) = (
                self.op[self.op[op2].prev].pt,
                self.op[op2].pt,
                self.op[self.op[op2].next].pt,
            );
            if is_collinear(p, c, n)
                && (c == p || c == n || !self.preserve_collinear || dot_product(p, c, n) < 0.0)
            {
                if Some(op2) == self.outrec_list[outrec].pts {
                    self.outrec_list[outrec].pts = Some(self.op[op2].prev);
                }
                op2 = self.dispose_out_pt(op2);
                if !self.is_valid_closed_path(Some(op2)) {
                    self.dispose_out_pts(outrec);
                    return;
                }
                start_op = op2;
                continue;
            }
            op2 = self.op[op2].next;
            if op2 == start_op {
                break;
            }
        }
        self.fix_self_intersects(outrec);
    }

    /// ClipperBase::DoSplitOp
    fn do_split_op(&mut self, outrec: usize, split_op: usize) {
        let prev_op = self.op[split_op].prev;
        let split_next = self.op[split_op].next;
        let next_next_op = self.op[split_next].next;
        self.outrec_list[outrec].pts = Some(prev_op);
        // the return value is ignored: parallel segments leave the default (0, 0)
        let ip = get_segment_intersect_pt(
            self.op[prev_op].pt,
            self.op[split_op].pt,
            self.op[split_next].pt,
            self.op[next_next_op].pt,
        )
        .unwrap_or_default();
        let area1 = self.area_op(prev_op);
        let abs_area1 = area1.abs();
        if abs_area1 < 2.0 {
            self.dispose_out_pts(outrec);
            return;
        }
        let area2 = area_triangle(ip, self.op[split_op].pt, self.op[split_next].pt);
        let abs_area2 = area2.abs();
        if ip == self.op[prev_op].pt || ip == self.op[next_next_op].pt {
            self.op[next_next_op].prev = prev_op;
            self.op[prev_op].next = next_next_op;
        } else {
            let o = self.op[prev_op].outrec;
            let new_op2 = self.new_out_pt(ip, o);
            self.op[new_op2].prev = prev_op;
            self.op[new_op2].next = next_next_op;
            self.op[next_next_op].prev = new_op2;
            self.op[prev_op].next = new_op2;
        }
        if abs_area2 >= 1.0 && (abs_area2 > abs_area1 || (area2 > 0.0) == (area1 > 0.0)) {
            let new_or = self.new_outrec();
            self.outrec_list[new_or].owner = self.outrec_list[outrec].owner;
            self.op[split_op].outrec = new_or;
            self.op[split_next].outrec = new_or;
            let new_op = self.new_out_pt(ip, new_or);
            self.op[new_op].prev = split_next;
            self.op[new_op].next = split_op;
            self.outrec_list[new_or].pts = Some(new_op);
            self.op[split_op].prev = new_op;
            self.op[split_next].next = new_op;
            if self.using_polytree {
                if self.path2_contains_path1_op(prev_op, new_op) {
                    self.outrec_list[new_or].splits = Some(vec![outrec]);
                } else {
                    self.outrec_list[outrec]
                        .splits
                        .get_or_insert_with(Vec::new)
                        .push(new_or);
                }
            }
        }
    }

    /// ClipperBase::FixSelfIntersects
    fn fix_self_intersects(&mut self, outrec: usize) {
        let Some(mut op2) = self.outrec_list[outrec].pts else {
            return;
        };
        if self.op[op2].prev == self.op[self.op[op2].next].next {
            return; // triangles can't self-intersect
        }
        loop {
            let n = self.op[op2].next;
            let nn = self.op[n].next;
            let nnn = self.op[nn].next;
            let p = self.op[op2].prev;
            if segments_intersect(
                self.op[p].pt,
                self.op[op2].pt,
                self.op[n].pt,
                self.op[nn].pt,
            ) {
                if segments_intersect(
                    self.op[p].pt,
                    self.op[op2].pt,
                    self.op[nn].pt,
                    self.op[nnn].pt,
                ) {
                    // adjacent intersections (a micro self-intersection)
                    op2 = self.duplicate_op(op2, false);
                    let nnn2 = self.op[self.op[self.op[op2].next].next].next;
                    self.op[op2].pt = self.op[nnn2].pt;
                    op2 = self.op[op2].next;
                } else {
                    let pts = self.outrec_list[outrec].pts;
                    if Some(op2) == pts || Some(n) == pts {
                        if let Some(p0) = pts {
                            self.outrec_list[outrec].pts = Some(self.op[p0].prev);
                        }
                    }
                    self.do_split_op(outrec, op2);
                    let Some(o) = self.outrec_list[outrec].pts else {
                        break;
                    };
                    op2 = o;
                    if self.op[op2].prev == self.op[self.op[op2].next].next {
                        break;
                    }
                    continue;
                }
            } else {
                op2 = self.op[op2].next;
            }
            if Some(op2) == self.outrec_list[outrec].pts {
                break;
            }
        }
    }

    /// ClipperBase::StartOpenPath
    fn start_open_path(&mut self, e: usize, pt: Point64) -> usize {
        let outrec = self.new_outrec();
        self.outrec_list[outrec].is_open = true;
        if self.ae[e].wind_dx > 0 {
            self.outrec_list[outrec].front_edge = Some(e);
            self.outrec_list[outrec].back_edge = None;
        } else {
            self.outrec_list[outrec].front_edge = None;
            self.outrec_list[outrec].back_edge = Some(e);
        }
        self.ae[e].outrec = Some(outrec);
        let op = self.new_out_pt(pt, outrec);
        self.outrec_list[outrec].pts = Some(op);
        op
    }

    /// Clipper2Lib::TrimHorz
    fn trim_horz(&mut self, horz_edge: usize, preserve_collinear: bool) {
        let mut was_trimmed = false;
        let mut pt = self.vertices[self.next_vertex(horz_edge)].pt;
        while pt.y == self.ae[horz_edge].top.y {
            let (bot, top) = (self.ae[horz_edge].bot, self.ae[horz_edge].top);
            if preserve_collinear && ((pt.x < top.x) != (bot.x < top.x)) {
                break;
            }
            self.ae[horz_edge].vertex_top = self.next_vertex(horz_edge);
            self.ae[horz_edge].top = pt;
            was_trimmed = true;
            if self.is_maxima(horz_edge) {
                break;
            }
            pt = self.vertices[self.next_vertex(horz_edge)].pt;
        }
        if was_trimmed {
            self.set_dx(horz_edge);
        }
    }

    /// ClipperBase::UpdateEdgeIntoAEL
    fn update_edge_into_ael(&mut self, e: usize) {
        self.ae[e].bot = self.ae[e].top;
        self.ae[e].vertex_top = self.next_vertex(e);
        self.ae[e].top = self.vertices[self.ae[e].vertex_top].pt;
        self.ae[e].curr_x = self.ae[e].bot.x;
        self.set_dx(e);
        if self.is_joined(e) {
            let bot = self.ae[e].bot;
            self.split(e, bot);
        }
        if self.is_horizontal(e) {
            if !self.is_open(e) {
                self.trim_horz(e, self.preserve_collinear);
            }
            return;
        }
        let ty = self.ae[e].top.y;
        self.insert_scanline(ty);
        let bot = self.ae[e].bot;
        self.check_join_left(e, bot, false);
        self.check_join_right(e, bot, true);
    }

    /// Clipper2Lib::FindEdgeWithMatchingLocMin
    fn find_edge_with_matching_loc_min(&self, e: usize) -> Option<usize> {
        let mut result = self.ae[e].next_in_ael;
        while let Some(r) = result {
            if self.ae[r].local_min == self.ae[e].local_min {
                return Some(r);
            } else if !self.is_horizontal(r) && self.ae[e].bot != self.ae[r].bot {
                result = None;
            } else {
                result = self.ae[r].next_in_ael;
            }
        }
        result = self.ae[e].prev_in_ael;
        while let Some(r) = result {
            if self.ae[r].local_min == self.ae[e].local_min {
                return Some(r);
            } else if !self.is_horizontal(r) && self.ae[e].bot != self.ae[r].bot {
                return None;
            } else {
                result = self.ae[r].prev_in_ael;
            }
        }
        result
    }

    /// ClipperBase::IntersectEdges
    fn intersect_edges(&mut self, e1: usize, e2: usize, pt: Point64) {
        // open path intersections
        if self.has_open_paths && (self.is_open(e1) || self.is_open(e2)) {
            if self.is_open(e1) && self.is_open(e2) {
                return;
            }
            let (edge_o, edge_c) = if self.is_open(e1) { (e1, e2) } else { (e2, e1) };
            if self.is_joined(edge_c) {
                self.split(edge_c, pt);
            }
            if self.ae[edge_c].wind_cnt.abs() != 1 {
                return;
            }
            match self.cliptype {
                ClipType::Union => {
                    if !self.is_hot_edge(edge_c) {
                        return;
                    }
                }
                _ => {
                    if self.minima_list[self.ae[edge_c].local_min].polytype == PathType::Subject {
                        return;
                    }
                }
            }
            match self.fillrule {
                FillRule::Positive => {
                    if self.ae[edge_c].wind_cnt != 1 {
                        return;
                    }
                }
                FillRule::Negative => {
                    if self.ae[edge_c].wind_cnt != -1 {
                        return;
                    }
                }
                _ => {
                    if self.ae[edge_c].wind_cnt.abs() != 1 {
                        return;
                    }
                }
            }
            if self.is_hot_edge(edge_o) {
                self.add_out_pt(edge_o, pt);
                let o = self.outrec_of(edge_o);
                if self.is_front(edge_o) {
                    self.outrec_list[o].front_edge = None;
                } else {
                    self.outrec_list[o].back_edge = None;
                }
                self.ae[edge_o].outrec = None;
            } else if pt == self.vertices[self.minima_list[self.ae[edge_o].local_min].vertex].pt
                && !self.is_open_end_v(self.minima_list[self.ae[edge_o].local_min].vertex)
            {
                let e3 = self.find_edge_with_matching_loc_min(edge_o);
                if let Some(e3) = e3.filter(|&x| self.is_hot_edge(x)) {
                    let o3 = self.outrec_of(e3);
                    self.ae[edge_o].outrec = Some(o3);
                    if self.ae[edge_o].wind_dx > 0 {
                        self.set_sides(o3, edge_o, e3);
                    } else {
                        self.set_sides(o3, e3, edge_o);
                    }
                    return;
                }
                self.start_open_path(edge_o, pt);
            } else {
                self.start_open_path(edge_o, pt);
            }
            return;
        }

        // closed paths
        if self.is_joined(e1) {
            self.split(e1, pt);
        }
        if self.is_joined(e2) {
            self.split(e2, pt);
        }

        let mut old_e1_windcnt;
        let old_e2_windcnt;
        if self.ae[e1].local_min != NIL && self.get_poly_type(e1) == self.get_poly_type(e2) {
            if self.fillrule == FillRule::EvenOdd {
                old_e1_windcnt = self.ae[e1].wind_cnt;
                self.ae[e1].wind_cnt = self.ae[e2].wind_cnt;
                self.ae[e2].wind_cnt = old_e1_windcnt;
            } else {
                if self.ae[e1].wind_cnt + self.ae[e2].wind_dx == 0 {
                    self.ae[e1].wind_cnt = -self.ae[e1].wind_cnt;
                } else {
                    self.ae[e1].wind_cnt += self.ae[e2].wind_dx;
                }
                if self.ae[e2].wind_cnt - self.ae[e1].wind_dx == 0 {
                    self.ae[e2].wind_cnt = -self.ae[e2].wind_cnt;
                } else {
                    self.ae[e2].wind_cnt -= self.ae[e1].wind_dx;
                }
            }
        } else if self.fillrule != FillRule::EvenOdd {
            self.ae[e1].wind_cnt2 += self.ae[e2].wind_dx;
            self.ae[e2].wind_cnt2 -= self.ae[e1].wind_dx;
        } else {
            self.ae[e1].wind_cnt2 = if self.ae[e1].wind_cnt2 == 0 { 1 } else { 0 };
            self.ae[e2].wind_cnt2 = if self.ae[e2].wind_cnt2 == 0 { 1 } else { 0 };
        }

        match self.fillrule {
            FillRule::EvenOdd | FillRule::NonZero => {
                old_e1_windcnt = self.ae[e1].wind_cnt.abs();
                old_e2_windcnt = self.ae[e2].wind_cnt.abs();
            }
            _ => {
                if self.fillrule == FillRule::Positive {
                    old_e1_windcnt = self.ae[e1].wind_cnt;
                    old_e2_windcnt = self.ae[e2].wind_cnt;
                } else {
                    old_e1_windcnt = -self.ae[e1].wind_cnt;
                    old_e2_windcnt = -self.ae[e2].wind_cnt;
                }
            }
        }

        let e1_windcnt_in_01 = old_e1_windcnt == 0 || old_e1_windcnt == 1;
        let e2_windcnt_in_01 = old_e2_windcnt == 0 || old_e2_windcnt == 1;
        if (!self.is_hot_edge(e1) && !e1_windcnt_in_01)
            || (!self.is_hot_edge(e2) && !e2_windcnt_in_01)
        {
            return;
        }

        if self.is_hot_edge(e1) && self.is_hot_edge(e2) {
            if (old_e1_windcnt != 0 && old_e1_windcnt != 1)
                || (old_e2_windcnt != 0 && old_e2_windcnt != 1)
                || (self.get_poly_type(e1) != self.get_poly_type(e2)
                    && self.cliptype != ClipType::Xor)
            {
                self.add_local_max_poly(e1, e2, pt);
            } else if self.is_front(e1) || self.ae[e1].outrec == self.ae[e2].outrec {
                self.add_local_max_poly(e1, e2, pt);
                self.add_local_min_poly(e1, e2, pt, false);
            } else {
                self.add_out_pt(e1, pt);
                self.add_out_pt(e2, pt);
                self.swap_outrecs(e1, e2);
            }
        } else if self.is_hot_edge(e1) {
            self.add_out_pt(e1, pt);
            self.swap_outrecs(e1, e2);
        } else if self.is_hot_edge(e2) {
            self.add_out_pt(e2, pt);
            self.swap_outrecs(e1, e2);
        } else {
            let (e1_wc2, e2_wc2) = match self.fillrule {
                FillRule::EvenOdd | FillRule::NonZero => (
                    i64::from(self.ae[e1].wind_cnt2.abs()),
                    i64::from(self.ae[e2].wind_cnt2.abs()),
                ),
                _ => {
                    if self.fillrule == FillRule::Positive {
                        (
                            i64::from(self.ae[e1].wind_cnt2),
                            i64::from(self.ae[e2].wind_cnt2),
                        )
                    } else {
                        (
                            -i64::from(self.ae[e1].wind_cnt2),
                            -i64::from(self.ae[e2].wind_cnt2),
                        )
                    }
                }
            };
            if !self.is_same_poly_type(e1, e2) {
                self.add_local_min_poly(e1, e2, pt, false);
            } else if old_e1_windcnt == 1 && old_e2_windcnt == 1 {
                match self.cliptype {
                    ClipType::Union => {
                        if e1_wc2 <= 0 && e2_wc2 <= 0 {
                            self.add_local_min_poly(e1, e2, pt, false);
                        }
                    }
                    ClipType::Difference => {
                        if (self.get_poly_type(e1) == PathType::Clip && e1_wc2 > 0 && e2_wc2 > 0)
                            || (self.get_poly_type(e1) == PathType::Subject
                                && e1_wc2 <= 0
                                && e2_wc2 <= 0)
                        {
                            self.add_local_min_poly(e1, e2, pt, false);
                        }
                    }
                    ClipType::Xor => {
                        self.add_local_min_poly(e1, e2, pt, false);
                    }
                    _ => {
                        if e1_wc2 > 0 && e2_wc2 > 0 {
                            self.add_local_min_poly(e1, e2, pt, false);
                        }
                    }
                }
            }
        }
    }

    /// ClipperBase::DeleteFromAEL (the edge's memory is not freed; nothing refers to it afterwards).
    fn delete_from_ael(&mut self, e: usize) {
        let prev = self.ae[e].prev_in_ael;
        let next = self.ae[e].next_in_ael;
        if prev.is_none() && next.is_none() && Some(e) != self.actives {
            return;
        }
        match prev {
            Some(p) => self.ae[p].next_in_ael = next,
            None => self.actives = next,
        }
        if let Some(n) = next {
            self.ae[n].prev_in_ael = prev;
        }
    }

    /// ClipperBase::AdjustCurrXAndCopyToSEL (1.5.4: no special case for left-joined edges)
    fn adjust_curr_x_and_copy_to_sel(&mut self, top_y: i64) {
        let mut e = self.actives;
        self.sel = e;
        while let Some(x) = e {
            self.ae[x].prev_in_sel = self.ae[x].prev_in_ael;
            self.ae[x].next_in_sel = self.ae[x].next_in_ael;
            self.ae[x].jump = self.ae[x].next_in_sel;
            self.ae[x].curr_x = self.top_x(x, top_y);
            e = self.ae[x].next_in_ael;
        }
    }

    /// ClipperBase::ExecuteInternal
    fn execute_internal(&mut self, ct: ClipType, fillrule: FillRule, use_polytrees: bool) -> bool {
        self.cliptype = ct;
        self.fillrule = fillrule;
        self.using_polytree = use_polytrees;
        self.reset();
        if ct == ClipType::NoClip {
            return true;
        }
        let Some(mut y) = self.pop_scanline() else {
            return true;
        };
        while self.succeeded {
            self.insert_local_minima_into_ael(y);
            while let Some(e) = self.pop_horz() {
                self.do_horizontal(e);
            }
            if !self.horz_seg_list.is_empty() {
                self.convert_horz_segs_to_joins();
                self.horz_seg_list.clear();
            }
            self.bot_y = y;
            match self.pop_scanline() {
                Some(ny) => y = ny,
                None => break,
            }
            self.do_intersections(y);
            self.do_top_of_scanbeam(y);
            while let Some(e) = self.pop_horz() {
                self.do_horizontal(e);
            }
        }
        if self.succeeded {
            self.process_horz_joins();
        }
        self.succeeded
    }

    /// Clipper2Lib::FixOutRecPts
    fn fix_outrec_pts(&mut self, outrec: usize) {
        let Some(start) = self.outrec_list[outrec].pts else {
            return;
        };
        let mut op = start;
        loop {
            self.op[op].outrec = outrec;
            op = self.op[op].next;
            if op == start {
                break;
            }
        }
    }

    /// Clipper2Lib::SetHorzSegHeadingForward
    fn set_horz_seg_heading_forward(&self, hs: &mut HorzSegment, op_p: usize, op_n: usize) -> bool {
        let (px, nx) = (self.op[op_p].pt.x, self.op[op_n].pt.x);
        if px == nx {
            return false;
        }
        if px < nx {
            hs.left_op = op_p;
            hs.right_op = Some(op_n);
            hs.left_to_right = true;
        } else {
            hs.left_op = op_n;
            hs.right_op = Some(op_p);
            hs.left_to_right = false;
        }
        true
    }

    /// Clipper2Lib::UpdateHorzSegment
    fn update_horz_segment(&mut self, i: usize) -> bool {
        let mut hs = self.horz_seg_list[i];
        let op = hs.left_op;
        let outrec = self
            .get_real_outrec(Some(self.op[op].outrec))
            .unwrap_or(NIL);
        let outrec_has_edges = self.outrec_list[outrec].front_edge.is_some();
        let curr_y = self.op[op].pt.y;
        let (mut op_p, mut op_n) = (op, op);
        if outrec_has_edges {
            let op_a = self.outrec_list[outrec].pts.unwrap_or(NIL);
            let op_z = self.op[op_a].next;
            while op_p != op_z && self.op[self.op[op_p].prev].pt.y == curr_y {
                op_p = self.op[op_p].prev;
            }
            while op_n != op_a && self.op[self.op[op_n].next].pt.y == curr_y {
                op_n = self.op[op_n].next;
            }
        } else {
            while self.op[op_p].prev != op_n && self.op[self.op[op_p].prev].pt.y == curr_y {
                op_p = self.op[op_p].prev;
            }
            while self.op[op_n].next != op_p && self.op[self.op[op_n].next].pt.y == curr_y {
                op_n = self.op[op_n].next;
            }
        }
        let result =
            self.set_horz_seg_heading_forward(&mut hs, op_p, op_n) && !self.op[hs.left_op].horz;
        if result {
            self.op[hs.left_op].horz = true;
        } else {
            hs.right_op = None;
        }
        self.horz_seg_list[i] = hs;
        result
    }

    /// ClipperBase::ConvertHorzSegsToJoins
    fn convert_horz_segs_to_joins(&mut self) {
        let mut j = 0;
        for i in 0..self.horz_seg_list.len() {
            if self.update_horz_segment(i) {
                j += 1;
            }
        }
        if j < 2 {
            return;
        }
        // std::stable_sort with HorzSegSorter: segments with a right end first, then by left x
        let ops = &self.op;
        self.horz_seg_list.sort_by(|a, b| {
            let less = |h1: &HorzSegment, h2: &HorzSegment| {
                if h1.right_op.is_none() || h2.right_op.is_none() {
                    h1.right_op.is_some()
                } else {
                    ops[h2.left_op].pt.x > ops[h1.left_op].pt.x
                }
            };
            if less(a, b) {
                std::cmp::Ordering::Less
            } else if less(b, a) {
                std::cmp::Ordering::Greater
            } else {
                std::cmp::Ordering::Equal
            }
        });
        for i1 in 0..j - 1 {
            for i2 in i1 + 1..j {
                let (hs1, hs2) = (self.horz_seg_list[i1], self.horz_seg_list[i2]);
                let (Some(r1), Some(r2)) = (hs1.right_op, hs2.right_op) else {
                    continue;
                };
                if self.op[hs2.left_op].pt.x >= self.op[r1].pt.x
                    || hs2.left_to_right == hs1.left_to_right
                    || self.op[r2].pt.x <= self.op[hs1.left_op].pt.x
                {
                    continue;
                }
                let curr_y = self.op[hs1.left_op].pt.y;
                let (mut l1, mut l2) = (hs1.left_op, hs2.left_op);
                if hs1.left_to_right {
                    while self.op[self.op[l1].next].pt.y == curr_y
                        && self.op[self.op[l1].next].pt.x <= self.op[l2].pt.x
                    {
                        l1 = self.op[l1].next;
                    }
                    while self.op[self.op[l2].prev].pt.y == curr_y
                        && self.op[self.op[l2].prev].pt.x <= self.op[l1].pt.x
                    {
                        l2 = self.op[l2].prev;
                    }
                    self.horz_seg_list[i1].left_op = l1;
                    self.horz_seg_list[i2].left_op = l2;
                    let op1 = self.duplicate_op(l1, true);
                    let op2 = self.duplicate_op(l2, false);
                    self.horz_join_list.push(HorzJoin { op1, op2 });
                } else {
                    while self.op[self.op[l1].prev].pt.y == curr_y
                        && self.op[self.op[l1].prev].pt.x <= self.op[l2].pt.x
                    {
                        l1 = self.op[l1].prev;
                    }
                    while self.op[self.op[l2].next].pt.y == curr_y
                        && self.op[self.op[l2].next].pt.x <= self.op[l1].pt.x
                    {
                        l2 = self.op[l2].next;
                    }
                    self.horz_seg_list[i1].left_op = l1;
                    self.horz_seg_list[i2].left_op = l2;
                    let op1 = self.duplicate_op(l2, true);
                    let op2 = self.duplicate_op(l1, false);
                    self.horz_join_list.push(HorzJoin { op1, op2 });
                }
            }
        }
    }

    /// Clipper2Lib::MoveSplits
    fn move_splits(&mut self, from_or: usize, to_or: usize) {
        let from = self.outrec_list[from_or].splits.take().unwrap_or_default();
        self.outrec_list[to_or]
            .splits
            .get_or_insert_with(Vec::new)
            .extend(from);
        self.outrec_list[from_or].splits = Some(Vec::new());
    }

    /// ClipperBase::ProcessHorzJoins
    fn process_horz_joins(&mut self) {
        for k in 0..self.horz_join_list.len() {
            let j = self.horz_join_list[k];
            let or1 = self
                .get_real_outrec(Some(self.op[j.op1].outrec))
                .unwrap_or(NIL);
            let mut or2 = self
                .get_real_outrec(Some(self.op[j.op2].outrec))
                .unwrap_or(NIL);
            let op1b = self.op[j.op1].next;
            let op2b = self.op[j.op2].prev;
            self.op[j.op1].next = j.op2;
            self.op[j.op2].prev = j.op1;
            self.op[op1b].prev = op2b;
            self.op[op2b].next = op1b;
            if or1 == or2 {
                // the join is really a split
                or2 = self.new_outrec();
                self.outrec_list[or2].pts = Some(op1b);
                self.fix_outrec_pts(or2);
                if let Some(p1) = self.outrec_list[or1].pts {
                    if self.op[p1].outrec == or2 {
                        self.outrec_list[or1].pts = Some(j.op1);
                        self.op[j.op1].outrec = or1;
                    }
                }
                if self.using_polytree {
                    let (p1, p2) = (
                        self.outrec_list[or1].pts.unwrap_or(NIL),
                        self.outrec_list[or2].pts.unwrap_or(NIL),
                    );
                    if self.path2_contains_path1_op(p1, p2) {
                        self.outrec_list[or1].pts = Some(p2);
                        self.outrec_list[or2].pts = Some(p1);
                        self.fix_outrec_pts(or1);
                        self.fix_outrec_pts(or2);
                        self.outrec_list[or2].owner = Some(or1);
                    } else if self.path2_contains_path1_op(p2, p1) {
                        self.outrec_list[or2].owner = Some(or1);
                    } else {
                        self.outrec_list[or2].owner = self.outrec_list[or1].owner;
                    }
                    self.outrec_list[or1]
                        .splits
                        .get_or_insert_with(Vec::new)
                        .push(or2);
                } else {
                    self.outrec_list[or2].owner = Some(or1);
                }
            } else {
                self.outrec_list[or2].pts = None;
                if self.using_polytree {
                    self.set_owner(or2, or1);
                    if self.outrec_list[or2].splits.is_some() {
                        self.move_splits(or2, or1);
                    }
                } else {
                    self.outrec_list[or2].owner = Some(or1);
                }
            }
        }
    }

    /// ClipperBase::DoIntersections
    fn do_intersections(&mut self, top_y: i64) {
        if self.build_intersect_list(top_y) {
            self.process_intersect_list();
            self.intersect_nodes.clear();
        }
    }

    /// ClipperBase::AddNewIntersectNode: the crossing of two edges, moved back into the scanbeam when rounding put it
    /// outside.
    fn add_new_intersect_node(&mut self, e1: usize, e2: usize, top_y: i64) {
        let (a1, a2) = (&self.ae[e1], &self.ae[e2]);
        let mut ip = get_segment_intersect_pt(a1.bot, a1.top, a2.bot, a2.top)
            .unwrap_or(Point64::new(a1.curr_x, top_y));
        if ip.y > self.bot_y || ip.y < top_y {
            let abs_dx1 = a1.dx.abs();
            let abs_dx2 = a2.dx.abs();
            if abs_dx1 > 100.0 && abs_dx2 > 100.0 {
                if abs_dx1 > abs_dx2 {
                    ip = get_closest_point_on_segment(ip, a1.bot, a1.top);
                } else {
                    ip = get_closest_point_on_segment(ip, a2.bot, a2.top);
                }
            } else if abs_dx1 > 100.0 {
                ip = get_closest_point_on_segment(ip, a1.bot, a1.top);
            } else if abs_dx2 > 100.0 {
                ip = get_closest_point_on_segment(ip, a2.bot, a2.top);
            } else {
                if ip.y < top_y {
                    ip.y = top_y;
                } else {
                    ip.y = self.bot_y;
                }
                if abs_dx1 < abs_dx2 {
                    ip.x = self.top_x(e1, ip.y);
                } else {
                    ip.x = self.top_x(e2, ip.y);
                }
            }
        }
        self.intersect_nodes.push(IntersectNode {
            pt: ip,
            edge1: e1,
            edge2: e2,
        });
    }

    /// ClipperBase::BuildIntersectList: a stable merge sort of the SEL by the edges' x at the top of the scanbeam,
    /// recording every pair that has to swap.
    fn build_intersect_list(&mut self, top_y: i64) -> bool {
        match self.actives {
            Some(a) if self.ae[a].next_in_ael.is_some() => {}
            _ => return false,
        }
        self.adjust_curr_x_and_copy_to_sel(top_y);
        let mut left = self.sel;
        while let Some(l0) = left.filter(|&l| self.ae[l].jump.is_some()) {
            let mut prev_base: Option<usize> = None;
            left = Some(l0);
            while let Some(l) = left.filter(|&l| self.ae[l].jump.is_some()) {
                let mut curr_base = l;
                let mut lft = l;
                let mut right = self.ae[l].jump;
                let mut l_end = right;
                let r_end = right.and_then(|r| self.ae[r].jump);
                self.ae[l].jump = r_end;
                while Some(lft) != l_end && right != r_end {
                    let Some(r) = right else {
                        break;
                    };
                    if self.ae[r].curr_x < self.ae[lft].curr_x {
                        let mut tmp = self.ae[r].prev_in_sel.unwrap_or(NIL);
                        loop {
                            self.add_new_intersect_node(tmp, r, top_y);
                            if tmp == lft {
                                break;
                            }
                            tmp = self.ae[tmp].prev_in_sel.unwrap_or(NIL);
                        }
                        let tmp = r;
                        right = self.extract_from_sel(tmp);
                        l_end = right;
                        self.insert1_before2_in_sel(tmp, lft);
                        if lft == curr_base {
                            curr_base = tmp;
                            self.ae[curr_base].jump = r_end;
                            match prev_base {
                                None => self.sel = Some(curr_base),
                                Some(pb) => self.ae[pb].jump = Some(curr_base),
                            }
                        }
                    } else {
                        match self.ae[lft].next_in_sel {
                            Some(n) => lft = n,
                            None => break,
                        }
                    }
                }
                prev_base = Some(curr_base);
                left = r_end;
            }
            left = self.sel;
        }
        !self.intersect_nodes.is_empty()
    }

    /// ClipperBase::ProcessIntersectList
    fn process_intersect_list(&mut self) {
        // std::sort with IntersectListSort: larger y first, then smaller x (stable here, see the module notes)
        self.intersect_nodes.sort_by(|a, b| {
            if a.pt.y == b.pt.y {
                a.pt.x.cmp(&b.pt.x)
            } else {
                b.pt.y.cmp(&a.pt.y)
            }
        });
        for i in 0..self.intersect_nodes.len() {
            if !self.edges_adjacent_in_ael(&self.intersect_nodes[i]) {
                let mut i2 = i + 1;
                while i2 < self.intersect_nodes.len()
                    && !self.edges_adjacent_in_ael(&self.intersect_nodes[i2])
                {
                    i2 += 1;
                }
                if i2 < self.intersect_nodes.len() {
                    self.intersect_nodes.swap(i, i2);
                }
            }
            let node = self.intersect_nodes[i];
            self.intersect_edges(node.edge1, node.edge2, node.pt);
            self.swap_positions_in_ael(node.edge1, node.edge2);
            self.ae[node.edge1].curr_x = node.pt.x;
            self.ae[node.edge2].curr_x = node.pt.x;
            self.check_join_left(node.edge2, node.pt, true);
            self.check_join_right(node.edge1, node.pt, true);
        }
    }

    /// ClipperBase::SwapPositionsInAEL (e1 must be immediately left of e2)
    fn swap_positions_in_ael(&mut self, e1: usize, e2: usize) {
        let next = self.ae[e2].next_in_ael;
        if let Some(n) = next {
            self.ae[n].prev_in_ael = Some(e1);
        }
        let prev = self.ae[e1].prev_in_ael;
        if let Some(p) = prev {
            self.ae[p].next_in_ael = Some(e2);
        }
        self.ae[e2].prev_in_ael = prev;
        self.ae[e2].next_in_ael = Some(e1);
        self.ae[e1].prev_in_ael = Some(e2);
        self.ae[e1].next_in_ael = next;
        if self.ae[e2].prev_in_ael.is_none() {
            self.actives = Some(e2);
        }
    }

    /// Clipper2Lib::GetLastOp
    fn get_last_op(&self, hot_edge: usize) -> usize {
        let outrec = self.outrec_of(hot_edge);
        let mut result = self.outrec_list[outrec].pts.unwrap_or(NIL);
        if self.outrec_list[outrec].front_edge != Some(hot_edge) {
            result = self.op[result].next;
        }
        result
    }

    /// ClipperBase::AddTrialHorzJoin
    fn add_trial_horz_join(&mut self, op: usize) {
        if self.outrec_list[self.op[op].outrec].is_open {
            return;
        }
        self.horz_seg_list.push(HorzSegment {
            left_op: op,
            right_op: None,
            left_to_right: true,
        });
    }

    /// ClipperBase::ResetHorzDirection
    fn reset_horz_direction(&self, horz: usize, max_vertex: Option<usize>) -> (bool, i64, i64) {
        let h = &self.ae[horz];
        if h.bot.x == h.top.x {
            let mut e = h.next_in_ael;
            while let Some(x) = e {
                if Some(self.ae[x].vertex_top) == max_vertex {
                    break;
                }
                e = self.ae[x].next_in_ael;
            }
            (e.is_some(), h.curr_x, h.curr_x)
        } else if h.curr_x < h.top.x {
            (true, h.curr_x, h.top.x)
        } else {
            (false, h.top.x, h.curr_x)
        }
    }

    /// ClipperBase::DoHorizontal
    fn do_horizontal(&mut self, horz: usize) {
        let horz_is_open = self.is_open(horz);
        let y = self.ae[horz].bot.y;
        let vertex_max = if horz_is_open {
            self.get_curr_y_maxima_vertex_open(horz)
        } else {
            self.get_curr_y_maxima_vertex(horz)
        };
        let (mut is_left_to_right, mut horz_left, mut horz_right) =
            self.reset_horz_direction(horz, vertex_max);
        if self.is_hot_edge(horz) {
            let cx = self.ae[horz].curr_x;
            let op = self.add_out_pt(horz, Point64::new(cx, y));
            self.add_trial_horz_join(op);
        }
        loop {
            let mut e = if is_left_to_right {
                self.ae[horz].next_in_ael
            } else {
                self.ae[horz].prev_in_ael
            };
            while let Some(ex) = e {
                if Some(self.ae[ex].vertex_top) == vertex_max {
                    if self.is_hot_edge(horz) && self.is_joined(ex) {
                        let t = self.ae[ex].top;
                        self.split(ex, t);
                    }
                    if self.is_hot_edge(horz) {
                        while Some(self.ae[horz].vertex_top) != vertex_max {
                            let t = self.ae[horz].top;
                            self.add_out_pt(horz, t);
                            self.update_edge_into_ael(horz);
                        }
                        let t = self.ae[horz].top;
                        if is_left_to_right {
                            self.add_local_max_poly(horz, ex, t);
                        } else {
                            self.add_local_max_poly(ex, horz, t);
                        }
                    }
                    self.delete_from_ael(ex);
                    self.delete_from_ael(horz);
                    return;
                }
                // if horz is a maxima, keep going until its maxima pair, otherwise check for break conditions
                if vertex_max != Some(self.ae[horz].vertex_top) || self.is_open_end(horz) {
                    if (is_left_to_right && self.ae[ex].curr_x > horz_right)
                        || (!is_left_to_right && self.ae[ex].curr_x < horz_left)
                    {
                        break;
                    }
                    if self.ae[ex].curr_x == self.ae[horz].top.x && !self.is_horizontal(ex) {
                        let pt = self.vertices[self.next_vertex(horz)].pt;
                        let open_cross = self.is_open(ex)
                            && !self.is_same_poly_type(ex, horz)
                            && !self.is_hot_edge(ex);
                        if is_left_to_right {
                            if open_cross {
                                if self.top_x(ex, pt.y) > pt.x {
                                    break;
                                }
                            } else if self.top_x(ex, pt.y) >= pt.x {
                                break;
                            }
                        } else if open_cross {
                            if self.top_x(ex, pt.y) < pt.x {
                                break;
                            }
                        } else if self.top_x(ex, pt.y) <= pt.x {
                            break;
                        }
                    }
                }
                let pt = Point64::new(self.ae[ex].curr_x, self.ae[horz].bot.y);
                if is_left_to_right {
                    self.intersect_edges(horz, ex, pt);
                    self.swap_positions_in_ael(horz, ex);
                    self.check_join_left(ex, pt, false);
                    self.ae[horz].curr_x = self.ae[ex].curr_x;
                    e = self.ae[horz].next_in_ael;
                } else {
                    self.intersect_edges(ex, horz, pt);
                    self.swap_positions_in_ael(ex, horz);
                    self.check_join_right(ex, pt, false);
                    self.ae[horz].curr_x = self.ae[ex].curr_x;
                    e = self.ae[horz].prev_in_ael;
                }
                if self.ae[horz].outrec.is_some() {
                    let last = self.get_last_op(horz);
                    self.add_trial_horz_join(last);
                }
            }
            // finished with (consecutive) horizontals?
            if horz_is_open && self.is_open_end(horz) {
                if self.is_hot_edge(horz) {
                    let t = self.ae[horz].top;
                    self.add_out_pt(horz, t);
                    let o = self.outrec_of(horz);
                    if self.is_front(horz) {
                        self.outrec_list[o].front_edge = None;
                    } else {
                        self.outrec_list[o].back_edge = None;
                    }
                    self.ae[horz].outrec = None;
                }
                self.delete_from_ael(horz);
                return;
            } else if self.vertices[self.next_vertex(horz)].pt.y != self.ae[horz].top.y {
                break;
            }
            // more horizontals in the bound
            if self.is_hot_edge(horz) {
                let t = self.ae[horz].top;
                self.add_out_pt(horz, t);
            }
            self.update_edge_into_ael(horz);
            (is_left_to_right, horz_left, horz_right) = self.reset_horz_direction(horz, vertex_max);
        }
        if self.is_hot_edge(horz) {
            let t = self.ae[horz].top;
            let op = self.add_out_pt(horz, t);
            self.add_trial_horz_join(op);
        }
        self.update_edge_into_ael(horz);
    }

    /// ClipperBase::DoTopOfScanbeam
    fn do_top_of_scanbeam(&mut self, y: i64) {
        self.sel = None;
        let mut e = self.actives;
        while let Some(x) = e {
            if self.ae[x].top.y == y {
                self.ae[x].curr_x = self.ae[x].top.x;
                if self.is_maxima(x) {
                    e = self.do_maxima(x);
                    continue;
                }
                if self.is_hot_edge(x) {
                    let t = self.ae[x].top;
                    self.add_out_pt(x, t);
                }
                self.update_edge_into_ael(x);
                if self.is_horizontal(x) {
                    self.push_horz(x);
                }
            } else {
                self.ae[x].curr_x = self.top_x(x, y);
            }
            e = self.ae[x].next_in_ael;
        }
    }

    /// ClipperBase::DoMaxima
    fn do_maxima(&mut self, e: usize) -> Option<usize> {
        let prev_e = self.ae[e].prev_in_ael;
        let mut next_e = self.ae[e].next_in_ael;
        if self.is_open_end(e) {
            if self.is_hot_edge(e) {
                let t = self.ae[e].top;
                self.add_out_pt(e, t);
            }
            if !self.is_horizontal(e) {
                if self.is_hot_edge(e) {
                    let o = self.outrec_of(e);
                    if self.is_front(e) {
                        self.outrec_list[o].front_edge = None;
                    } else {
                        self.outrec_list[o].back_edge = None;
                    }
                    self.ae[e].outrec = None;
                }
                self.delete_from_ael(e);
            }
            return next_e;
        }
        let Some(max_pair) = self.get_maxima_pair(e) else {
            return next_e; // the maxima pair is horizontal
        };
        if self.is_joined(e) {
            let t = self.ae[e].top;
            self.split(e, t);
        }
        if self.is_joined(max_pair) {
            let t = self.ae[max_pair].top;
            self.split(max_pair, t);
        }
        while next_e != Some(max_pair) {
            let Some(n) = next_e else {
                break;
            };
            let t = self.ae[e].top;
            self.intersect_edges(e, n, t);
            self.swap_positions_in_ael(e, n);
            next_e = self.ae[e].next_in_ael;
        }
        if self.is_open(e) {
            if self.is_hot_edge(e) {
                let t = self.ae[e].top;
                self.add_local_max_poly(e, max_pair, t);
            }
            self.delete_from_ael(max_pair);
            self.delete_from_ael(e);
            return match prev_e {
                Some(p) => self.ae[p].next_in_ael,
                None => self.actives,
            };
        }
        if self.is_hot_edge(e) {
            let t = self.ae[e].top;
            self.add_local_max_poly(e, max_pair, t);
        }
        self.delete_from_ael(e);
        self.delete_from_ael(max_pair);
        match prev_e {
            Some(p) => self.ae[p].next_in_ael,
            None => self.actives,
        }
    }

    /// ClipperBase::Split
    fn split(&mut self, e: usize, pt: Point64) {
        if self.ae[e].join_with == JoinWith::Right {
            self.ae[e].join_with = JoinWith::NoJoin;
            let n = self.ae[e].next_in_ael.unwrap_or(NIL);
            self.ae[n].join_with = JoinWith::NoJoin;
            self.add_local_min_poly(e, n, pt, true);
        } else {
            self.ae[e].join_with = JoinWith::NoJoin;
            let p = self.ae[e].prev_in_ael.unwrap_or(NIL);
            self.ae[p].join_with = JoinWith::NoJoin;
            self.add_local_min_poly(p, e, pt, true);
        }
    }

    /// ClipperBase::CheckJoinLeft
    fn check_join_left(&mut self, e: usize, pt: Point64, check_curr_x: bool) {
        let Some(prev) = self.ae[e].prev_in_ael else {
            return;
        };
        if !self.is_hot_edge(e)
            || !self.is_hot_edge(prev)
            || self.is_horizontal(e)
            || self.is_horizontal(prev)
            || self.is_open(e)
            || self.is_open(prev)
        {
            return;
        }
        let (a, b) = (&self.ae[e], &self.ae[prev]);
        if (pt.y < a.top.y + 2 || pt.y < b.top.y + 2) && (a.bot.y > pt.y || b.bot.y > pt.y) {
            return; // avoid trivial joins
        }
        if check_curr_x {
            if perpendic_dist_from_line_sqrd(pt, b.bot, b.top) > 0.25 {
                return;
            }
        } else if a.curr_x != b.curr_x {
            return;
        }
        if !is_collinear(a.top, pt, b.top) {
            return;
        }
        let (oe, op_) = (self.outrec_of(e), self.outrec_of(prev));
        if self.outrec_list[oe].idx == self.outrec_list[op_].idx {
            self.add_local_max_poly(prev, e, pt);
        } else if self.outrec_list[oe].idx < self.outrec_list[op_].idx {
            self.join_outrec_paths(e, prev);
        } else {
            self.join_outrec_paths(prev, e);
        }
        self.ae[prev].join_with = JoinWith::Right;
        self.ae[e].join_with = JoinWith::Left;
    }

    /// ClipperBase::CheckJoinRight
    fn check_join_right(&mut self, e: usize, pt: Point64, check_curr_x: bool) {
        let Some(next) = self.ae[e].next_in_ael else {
            return;
        };
        if !self.is_hot_edge(e)
            || !self.is_hot_edge(next)
            || self.is_horizontal(e)
            || self.is_horizontal(next)
            || self.is_open(e)
            || self.is_open(next)
        {
            return;
        }
        let (a, b) = (&self.ae[e], &self.ae[next]);
        if (pt.y < a.top.y + 2 || pt.y < b.top.y + 2) && (a.bot.y > pt.y || b.bot.y > pt.y) {
            return; // avoid trivial joins
        }
        if check_curr_x {
            if perpendic_dist_from_line_sqrd(pt, b.bot, b.top) > 0.35 {
                return;
            }
        } else if a.curr_x != b.curr_x {
            return;
        }
        if !is_collinear(a.top, pt, b.top) {
            return;
        }
        let (oe, on) = (self.outrec_of(e), self.outrec_of(next));
        if self.outrec_list[oe].idx == self.outrec_list[on].idx {
            self.add_local_max_poly(e, next, pt);
        } else if self.outrec_list[oe].idx < self.outrec_list[on].idx {
            self.join_outrec_paths(e, next);
        } else {
            self.join_outrec_paths(next, e);
        }
        self.ae[e].join_with = JoinWith::Right;
        self.ae[next].join_with = JoinWith::Left;
    }

    /// Clipper2Lib::BuildPath64
    fn build_path64(
        &self,
        op: Option<usize>,
        reverse: bool,
        is_open: bool,
        path: &mut Path64,
    ) -> bool {
        let Some(mut op) = op else {
            return false;
        };
        if self.op[op].next == op || (!is_open && self.op[op].next == self.op[op].prev) {
            return false;
        }
        path.clear();
        let mut last_pt;
        let mut op2;
        if reverse {
            last_pt = self.op[op].pt;
            op2 = self.op[op].prev;
        } else {
            op = self.op[op].next;
            last_pt = self.op[op].pt;
            op2 = self.op[op].next;
        }
        path.push(last_pt);
        while op2 != op {
            if self.op[op2].pt != last_pt {
                last_pt = self.op[op2].pt;
                path.push(last_pt);
            }
            op2 = if reverse {
                self.op[op2].prev
            } else {
                self.op[op2].next
            };
        }
        !(!is_open && path.len() == 3 && self.is_very_small_triangle(op2))
    }

    /// ClipperBase::CheckBounds
    fn check_bounds(&mut self, outrec: usize) -> bool {
        if self.outrec_list[outrec].pts.is_none() {
            return false;
        }
        if !self.outrec_list[outrec].bounds.is_empty() {
            return true;
        }
        self.clean_collinear(outrec);
        let mut path = Vec::new();
        if self.outrec_list[outrec].pts.is_none()
            || !self.build_path64(
                self.outrec_list[outrec].pts,
                self.reverse_solution,
                false,
                &mut path,
            )
        {
            return false;
        }
        self.outrec_list[outrec].bounds = get_bounds(&path);
        self.outrec_list[outrec].path = path;
        true
    }

    /// ClipperBase::CheckSplitOwner
    fn check_split_owner(&mut self, outrec: usize, splits: &[usize]) -> bool {
        for &split0 in splits {
            if self.outrec_list[split0].pts.is_none() {
                if let Some(s2) = self.outrec_list[split0].splits.clone() {
                    if self.check_split_owner(outrec, &s2) {
                        return true;
                    }
                }
            }
            let Some(split) = self.get_real_outrec(Some(split0)) else {
                continue;
            };
            if split == outrec || self.outrec_list[split].recursive_split == Some(outrec) {
                continue;
            }
            self.outrec_list[split].recursive_split = Some(outrec);
            if let Some(s2) = self.outrec_list[split].splits.clone() {
                if self.check_split_owner(outrec, &s2) {
                    return true;
                }
            }
            if !self.check_bounds(split) {
                continue;
            }
            let (sb, ob) = (
                self.outrec_list[split].bounds,
                self.outrec_list[outrec].bounds,
            );
            if !sb.contains(&ob) {
                continue;
            }
            let (op_o, op_s) = (
                self.outrec_list[outrec].pts.unwrap_or(NIL),
                self.outrec_list[split].pts.unwrap_or(NIL),
            );
            if !self.path2_contains_path1_op(op_o, op_s) {
                continue;
            }
            if !self.is_valid_owner(outrec, Some(split)) {
                self.outrec_list[split].owner = self.outrec_list[outrec].owner;
            }
            self.outrec_list[outrec].owner = Some(split);
            return true;
        }
        false
    }

    /// ClipperBase::RecursiveCheckOwners
    fn recursive_check_owners(&mut self, outrec: usize, tree: &mut PolyTree64, polypath: usize) {
        if self.outrec_list[outrec].polypath.is_some() || self.outrec_list[outrec].bounds.is_empty()
        {
            return;
        }
        while let Some(owner) = self.outrec_list[outrec].owner {
            if let Some(s) = self.outrec_list[owner].splits.clone() {
                if self.check_split_owner(outrec, &s) {
                    break;
                }
            }
            let owner = self.outrec_list[outrec].owner.unwrap_or(owner);
            if self.outrec_list[owner].pts.is_some() && self.check_bounds(owner) {
                let (b_owner, b_out) = (
                    self.outrec_list[owner].bounds,
                    self.outrec_list[outrec].bounds,
                );
                let (op_o, op_w) = (
                    self.outrec_list[outrec].pts.unwrap_or(NIL),
                    self.outrec_list[owner].pts.unwrap_or(NIL),
                );
                if b_owner.contains(&b_out) && self.path2_contains_path1_op(op_o, op_w) {
                    break;
                }
            }
            self.outrec_list[outrec].owner = self.outrec_list[owner].owner;
        }
        if let Some(owner) = self.outrec_list[outrec].owner {
            if self.outrec_list[owner].polypath.is_none() {
                self.recursive_check_owners(owner, tree, polypath);
            }
            let parent = self.outrec_list[owner].polypath.unwrap_or(polypath);
            let child = tree.add_child(parent, &self.outrec_list[outrec].path);
            self.outrec_list[outrec].polypath = Some(child);
        } else {
            let child = tree.add_child(polypath, &self.outrec_list[outrec].path);
            self.outrec_list[outrec].polypath = Some(child);
        }
    }

    /// Clipper64::BuildPaths64
    fn build_paths64(
        &mut self,
        solution_closed: &mut Paths64,
        mut solution_open: Option<&mut Paths64>,
    ) {
        solution_closed.clear();
        if let Some(so) = solution_open.as_deref_mut() {
            so.clear();
        }
        let mut i = 0;
        while i < self.outrec_list.len() {
            let outrec = i;
            i += 1;
            if self.outrec_list[outrec].pts.is_none() {
                continue;
            }
            let mut path = Vec::new();
            if solution_open.is_some() && self.outrec_list[outrec].is_open {
                if self.build_path64(
                    self.outrec_list[outrec].pts,
                    self.reverse_solution,
                    true,
                    &mut path,
                ) {
                    if let Some(so) = solution_open.as_deref_mut() {
                        so.push(path);
                    }
                }
            } else {
                self.clean_collinear(outrec);
                if self.build_path64(
                    self.outrec_list[outrec].pts,
                    self.reverse_solution,
                    false,
                    &mut path,
                ) {
                    solution_closed.push(path);
                }
            }
        }
    }

    /// Clipper64::BuildTree64
    fn build_tree64(&mut self, polytree: &mut PolyTree64, open_paths: &mut Paths64) {
        polytree.clear();
        open_paths.clear();
        let mut i = 0;
        while i < self.outrec_list.len() {
            let outrec = i;
            i += 1;
            if self.outrec_list[outrec].pts.is_none() {
                continue;
            }
            if self.outrec_list[outrec].is_open {
                let mut path = Vec::new();
                if self.build_path64(
                    self.outrec_list[outrec].pts,
                    self.reverse_solution,
                    true,
                    &mut path,
                ) {
                    open_paths.push(path);
                }
                continue;
            }
            if self.check_bounds(outrec) {
                self.recursive_check_owners(outrec, polytree, 0);
            }
        }
    }

    /// Clipper64::Execute (closed and open paths)
    pub fn execute_paths(
        &mut self,
        clip_type: ClipType,
        fill_rule: FillRule,
        closed_paths: &mut Paths64,
        open_paths: &mut Paths64,
    ) -> bool {
        closed_paths.clear();
        open_paths.clear();
        if self.execute_internal(clip_type, fill_rule, false) {
            self.build_paths64(closed_paths, Some(open_paths));
        }
        self.clean_up();
        self.succeeded
    }

    /// Clipper64::Execute (PolyTree64)
    pub fn execute_tree(
        &mut self,
        clip_type: ClipType,
        fill_rule: FillRule,
        polytree: &mut PolyTree64,
        open_paths: &mut Paths64,
    ) -> bool {
        if self.execute_internal(clip_type, fill_rule, true) {
            open_paths.clear();
            polytree.clear();
            self.build_tree64(polytree, open_paths);
        }
        self.clean_up();
        self.succeeded
    }
}

/// Clipper2Lib::GetDx
fn get_dx(pt1: Point64, pt2: Point64) -> f64 {
    let dy = (pt2.y - pt1.y) as f64;
    if dy != 0.0 {
        (pt2.x - pt1.x) as f64 / dy
    } else if pt2.x > pt1.x {
        -f64::MAX
    } else {
        f64::MAX
    }
}

/// Clipper2Lib::PtsReallyClose
fn pts_really_close(pt1: Point64, pt2: Point64) -> bool {
    (pt1.x - pt2.x).abs() < 2 && (pt1.y - pt2.y).abs() < 2
}

/// Clipper2Lib::AreaTriangle
fn area_triangle(pt1: Point64, pt2: Point64, pt3: Point64) -> f64 {
    (pt3.y + pt1.y) as f64 * (pt3.x - pt1.x) as f64
        + (pt1.y + pt2.y) as f64 * (pt1.x - pt2.x) as f64
        + (pt2.y + pt3.y) as f64 * (pt2.x - pt3.x) as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(x0: i64, y0: i64, x1: i64, y1: i64) -> Path64 {
        vec![
            Point64::new(x0, y0),
            Point64::new(x1, y0),
            Point64::new(x1, y1),
            Point64::new(x0, y1),
        ]
    }

    fn run(ct: ClipType, subj: Path64, clip: Path64) -> Paths64 {
        let mut c = Clipper64::new();
        c.add_subject(&[subj]);
        c.add_clip(&[clip]);
        let (mut closed, mut open) = (Vec::new(), Vec::new());
        assert!(c.execute_paths(ct, FillRule::NonZero, &mut closed, &mut open));
        assert!(open.is_empty());
        closed
    }

    fn sorted(p: &Path64) -> Vec<(i64, i64)> {
        let mut v: Vec<(i64, i64)> = p.iter().map(|p| (p.x, p.y)).collect();
        v.sort_unstable();
        v
    }

    /// Two 10x10 squares overlapping in a 5x5 corner: the union is one octagon-shaped outline of the 8 outer corners
    /// and the two crossings, the intersection is the shared 5x5 square, the difference is the first square without it.
    #[test]
    fn overlapping_squares_give_the_textbook_results() {
        let (a, b) = (square(0, 0, 10, 10), square(5, 5, 15, 15));
        let u = run(ClipType::Union, a.clone(), b.clone());
        assert_eq!(u.len(), 1);
        assert_eq!(
            sorted(&u[0]),
            vec![
                (0, 0),
                (0, 10),
                (5, 10),
                (5, 15),
                (10, 0),
                (10, 5),
                (15, 5),
                (15, 15)
            ]
        );
        let i = run(ClipType::Intersection, a.clone(), b.clone());
        assert_eq!(i.len(), 1);
        assert_eq!(sorted(&i[0]), vec![(5, 5), (5, 10), (10, 5), (10, 10)]);
        let d = run(ClipType::Difference, a, b);
        assert_eq!(d.len(), 1);
        assert_eq!(
            sorted(&d[0]),
            vec![(0, 0), (0, 10), (5, 5), (5, 10), (10, 0), (10, 5)]
        );
    }

    /// A square with a square hole (the hole wound the other way) under the positive rule: the tree holds the outer
    /// ring at the top and the hole as its child.
    #[test]
    fn a_hole_becomes_a_child_in_the_tree() {
        let outer = square(0, 0, 30, 30);
        let mut hole = square(10, 10, 20, 20);
        hole.reverse();
        let mut c = Clipper64::new();
        c.add_subject(&[outer, hole]);
        let mut tree = PolyTree64::default();
        let mut open = Vec::new();
        assert!(c.execute_tree(ClipType::Union, FillRule::NonZero, &mut tree, &mut open));
        let top = tree.children(0);
        assert_eq!(top.len(), 1);
        assert_eq!(
            sorted(&tree.nodes[top[0]].polygon),
            vec![(0, 0), (0, 30), (30, 0), (30, 30)]
        );
        let holes = tree.children(top[0]);
        assert_eq!(holes.len(), 1);
        assert_eq!(
            sorted(&tree.nodes[holes[0]].polygon),
            vec![(10, 10), (10, 20), (20, 10), (20, 20)]
        );
    }
}
