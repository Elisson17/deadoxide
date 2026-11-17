//! The `STDT` chunk of `ZomboidSound.strings.bank`: the table that gives every object of the FMOD Studio project (events,
//! snapshots, buses, VCAs, banks, parameters) its path. Reverse engineered; the layout below reproduces all 3490 event
//! and snapshot paths of the booted original with their GUIDs (case `fmodstrings`), and the 117 parameter names:
//!
//! ```text
//! u32 version (1)   u32 UNKNOWN (535661 in this bank)
//! nodes[N]          8 bytes: u24 x, u8 c, u24 y, u8 n   (a path trie, root = node 0)
//! u32 UNKNOWN (0x101ddf)
//! guids[G]          16 bytes each (the GUID of the object of leaf value 0..G)
//! u32 pool length (low 16 bits; the high bits are 1)
//! pool              NUL-terminated labels
//! u16 G   leaf[G]   u24: the node of the leaf with value i
//! u16 N   parent[N] u24: the parent of node i (0xffffff for the root)
//! ```
//!
//! A node's label is the NUL-terminated text at `pool[x]` (the whole label, case kept; `x == 0xffffff` is the empty label
//! of the root and of the terminator child), `c` is the lower-cased first character of the label (0 for a terminator)
//! the lookup compares, `n > 0` gives `n` children at node indices `y..y + n` (sorted by `c`), and `n == 0` is a leaf
//! whose value `y` indexes `guids`. A path that is a prefix of another ends in a terminator child (`c == 0`).
use std::collections::HashMap;

use super::riff::u32le;

#[derive(Clone, Copy, Debug)]
struct Node {
    x: u32,
    c: u8,
    y: u32,
    n: u8,
}

const NO_LABEL: u32 = 0xff_ffff;

#[derive(Clone, Debug)]
pub struct StringTable {
    nodes: Vec<Node>,
    guids: Vec<[u8; 16]>,
    pool: Vec<u8>,
    leaf_of: Vec<u32>,
    parent: Vec<u32>,
    by_guid: HashMap<[u8; 16], usize>,
    /// the two UNKNOWN u32 of the head and the one before the GUID table
    pub unknown: [u32; 3],
}

fn u24(d: &[u8], at: usize) -> Result<u32, String> {
    d.get(at..at + 3)
        .map(|b| u32::from(b[0]) | u32::from(b[1]) << 8 | u32::from(b[2]) << 16)
        .ok_or_else(|| "STDT: read past the end".to_string())
}

impl StringTable {
    pub fn parse(body: &[u8]) -> Result<StringTable, String> {
        let len = body.len();
        let version = u32le(body, 0)?;
        if version != 1 {
            return Err(format!("STDT: version {version}, only 1 is known"));
        }
        let node_at = |i: usize| -> Result<Node, String> {
            let a = u32le(body, 8 + 8 * i)?;
            let b = u32le(body, 12 + 8 * i)?;
            Ok(Node {
                x: a & 0xff_ffff,
                c: (a >> 24) as u8,
                y: b & 0xff_ffff,
                n: (b >> 24) as u8,
            })
        };
        // the extent of the node array and the GUID table: what the trie from the root reaches
        let max_nodes = len / 8;
        let (mut n_nodes, mut n_guids) = (1usize, 0usize);
        let mut stack = vec![0usize];
        let mut seen = 0usize;
        while let Some(i) = stack.pop() {
            seen += 1;
            if seen > max_nodes {
                return Err("STDT: the trie has a cycle".into());
            }
            let nd = node_at(i)?;
            n_nodes = n_nodes.max(i + 1);
            if nd.n == 0 {
                n_guids = n_guids.max(nd.y as usize + 1);
            } else {
                for k in 0..nd.n as usize {
                    stack.push(nd.y as usize + k);
                }
            }
        }
        let nodes_end = 8 + 8 * n_nodes;
        let tab = nodes_end + 4;
        let tab_end = tab + 16 * n_guids;
        let pool_at = tab_end + 4;
        let tail_len = 2 + 3 * n_guids + 2 + 3 * n_nodes;
        if pool_at + tail_len > len {
            return Err("STDT: the tables do not fit the chunk".into());
        }
        let pool_len = len - tail_len - pool_at;
        let pool_mark = u32le(body, tab_end)?;
        if pool_mark & 0xffff != (pool_len & 0xffff) as u32 || pool_mark >> 16 != 1 {
            return Err(format!(
                "STDT: pool header {pool_mark:#x} does not match the pool length {pool_len}"
            ));
        }
        let tail = pool_at + pool_len;
        let g = usize::from(body[tail]) | usize::from(body[tail + 1]) << 8;
        let n = usize::from(body[tail + 2 + 3 * n_guids])
            | usize::from(body[tail + 3 + 3 * n_guids]) << 8;
        if g != n_guids || n != n_nodes {
            return Err(format!(
                "STDT: tail counts {g}/{n} differ from the trie's {n_guids}/{n_nodes}"
            ));
        }
        let mut nodes = Vec::with_capacity(n_nodes);
        for i in 0..n_nodes {
            nodes.push(node_at(i)?);
        }
        let mut guids = Vec::with_capacity(n_guids);
        for i in 0..n_guids {
            let mut gd = [0u8; 16];
            gd.copy_from_slice(&body[tab + 16 * i..tab + 16 * i + 16]);
            guids.push(gd);
        }
        let mut leaf_of = Vec::with_capacity(n_guids);
        for i in 0..n_guids {
            leaf_of.push(u24(body, tail + 2 + 3 * i)?);
        }
        let mut parent = Vec::with_capacity(n_nodes);
        for i in 0..n_nodes {
            parent.push(u24(body, tail + 4 + 3 * n_guids + 3 * i)?);
        }
        let by_guid = guids.iter().enumerate().map(|(i, g)| (*g, i)).collect();
        let t = StringTable {
            nodes,
            guids,
            pool: body[pool_at..pool_at + pool_len].to_vec(),
            leaf_of,
            parent,
            by_guid,
            unknown: [u32le(body, 4)?, u32le(body, nodes_end)?, pool_mark],
        };
        t.check()?;
        Ok(t)
    }

    /// The tail arrays against the trie: the parent of every node and the node of every leaf value.
    fn check(&self) -> Result<(), String> {
        if self.parent.first() != Some(&NO_LABEL) {
            return Err("STDT: the root has a parent".into());
        }
        for (i, nd) in self.nodes.iter().enumerate() {
            if nd.n == 0 {
                if self.leaf_of.get(nd.y as usize) != Some(&(i as u32)) {
                    return Err(format!("STDT: leaf table disagrees at node {i}"));
                }
            } else {
                for k in 0..nd.n as usize {
                    if self.parent.get(nd.y as usize + k) != Some(&(i as u32)) {
                        return Err(format!(
                            "STDT: parent table disagrees at node {}",
                            nd.y as usize + k
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    fn label(&self, nd: &Node) -> Result<&[u8], String> {
        if nd.x == NO_LABEL {
            return Ok(&[]);
        }
        let at = nd.x as usize;
        let rest = self.pool.get(at..).ok_or("STDT: label past the pool")?;
        let e = rest
            .iter()
            .position(|b| *b == 0)
            .ok_or("STDT: label without a terminator")?;
        Ok(&rest[..e])
    }

    /// The number of strings (leaves, GUIDs).
    pub fn len(&self) -> usize {
        self.guids.len()
    }

    pub fn is_empty(&self) -> bool {
        self.guids.is_empty()
    }

    /// The path of the leaf with value `i`: the labels from the root down, found by walking the parent table up.
    pub fn path_of_index(&self, i: usize) -> Result<String, String> {
        let mut at = *self.leaf_of.get(i).ok_or("STDT: no such string")? as usize;
        let mut parts: Vec<&[u8]> = Vec::new();
        loop {
            let nd = self.nodes.get(at).ok_or("STDT: node out of range")?;
            parts.push(self.label(nd)?);
            if at == 0 {
                break;
            }
            at = self.parent[at] as usize;
        }
        parts.reverse();
        String::from_utf8(parts.concat()).map_err(|e| format!("STDT: path is not UTF-8: {e}"))
    }

    /// Every string with its GUID, in leaf-value order.
    pub fn entries(&self) -> Result<Vec<(String, [u8; 16])>, String> {
        (0..self.len())
            .map(|i| Ok((self.path_of_index(i)?, self.guids[i])))
            .collect()
    }

    /// `FMOD::Studio::System::lookupPath`: the path of an object's GUID.
    pub fn lookup_path(&self, guid: &[u8; 16]) -> Option<String> {
        self.by_guid
            .get(guid)
            .and_then(|i| self.path_of_index(*i).ok())
    }

    /// `FMOD::Studio::System::lookupID`: the GUID of a path, compared without regard to ASCII case.
    pub fn lookup_id(&self, path: &str) -> Option<[u8; 16]> {
        let s: Vec<u8> = path.bytes().map(|b| b.to_ascii_lowercase()).collect();
        let (mut at, mut pos) = (0usize, 0usize);
        loop {
            let nd = self.nodes.get(at)?;
            let label = self.label(nd).ok()?;
            let end = pos + label.len();
            if end > s.len()
                || !label
                    .iter()
                    .zip(&s[pos..end])
                    .all(|(a, b)| a.to_ascii_lowercase() == *b)
            {
                return None;
            }
            pos = end;
            if nd.n == 0 {
                return (pos == s.len()).then(|| self.guids[nd.y as usize]);
            }
            let key = s.get(pos).copied().unwrap_or(0);
            at = (0..nd.n as usize)
                .map(|k| nd.y as usize + k)
                .find(|k| self.nodes.get(*k).is_some_and(|c| c.c == key))?;
        }
    }
}
