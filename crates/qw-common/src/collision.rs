use crate::defs::{CONTENTS_EMPTY, CONTENTS_SOLID};
use crate::types::Vec3;

const DIST_EPSILON: f32 = 0.03125;

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Plane {
    pub normal: Vec3,
    pub dist: f32,
    pub plane_type: i32,
    pub signbits: u8,
}

impl Default for Plane {
    fn default() -> Self {
        Self {
            normal: Vec3::default(),
            dist: 0.0,
            plane_type: 0,
            signbits: 0,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct ClipNode {
    pub planenum: i32,
    pub children: [i32; 2],
}

#[derive(Debug, Copy, Clone)]
pub struct Hull<'a> {
    pub clipnodes: &'a [ClipNode],
    pub planes: &'a [Plane],
    pub firstclipnode: i32,
    pub lastclipnode: i32,
    pub clip_mins: Vec3,
    pub clip_maxs: Vec3,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Trace {
    pub allsolid: bool,
    pub startsolid: bool,
    pub inopen: bool,
    pub inwater: bool,
    pub fraction: f32,
    pub endpos: Vec3,
    pub plane: Plane,
}

impl Default for Trace {
    fn default() -> Self {
        Self {
            allsolid: false,
            startsolid: false,
            inopen: false,
            inwater: false,
            fraction: 1.0,
            endpos: Vec3::default(),
            plane: Plane::default(),
        }
    }
}

pub struct BoxHull {
    planes: [Plane; 6],
    clipnodes: [ClipNode; 6],
    clip_mins: Vec3,
    clip_maxs: Vec3,
}

impl BoxHull {
    pub fn new(mins: Vec3, maxs: Vec3) -> Self {
        let mut planes = [Plane::default(); 6];
        let mut clipnodes = [ClipNode {
            planenum: 0,
            children: [0, 0],
        }; 6];

        for i in 0..6 {
            clipnodes[i].planenum = i as i32;
            let side = i & 1;
            clipnodes[i].children[side] = CONTENTS_EMPTY;
            clipnodes[i].children[side ^ 1] = if i != 5 {
                (i + 1) as i32
            } else {
                CONTENTS_SOLID
            };

            let axis = i >> 1;
            planes[i].plane_type = axis as i32;
            let mut normal = Vec3::default();
            match axis {
                0 => normal.x = 1.0,
                1 => normal.y = 1.0,
                _ => normal.z = 1.0,
            }
            planes[i].normal = normal;
        }

        planes[0].dist = maxs.x;
        planes[1].dist = mins.x;
        planes[2].dist = maxs.y;
        planes[3].dist = mins.y;
        planes[4].dist = maxs.z;
        planes[5].dist = mins.z;

        Self {
            planes,
            clipnodes,
            clip_mins: mins,
            clip_maxs: maxs,
        }
    }

    pub fn hull(&self) -> Hull<'_> {
        Hull {
            clipnodes: &self.clipnodes,
            planes: &self.planes,
            firstclipnode: 0,
            lastclipnode: 5,
            clip_mins: self.clip_mins,
            clip_maxs: self.clip_maxs,
        }
    }
}

pub fn hull_point_contents(hull: &Hull<'_>, mut num: i32, point: Vec3) -> i32 {
    while num >= 0 {
        if num < hull.firstclipnode || num > hull.lastclipnode {
            return CONTENTS_SOLID;
        }
        let node = &hull.clipnodes[num as usize];
        let plane = &hull.planes[node.planenum as usize];
        let d = if plane.plane_type < 3 {
            match plane.plane_type {
                0 => point.x - plane.dist,
                1 => point.y - plane.dist,
                _ => point.z - plane.dist,
            }
        } else {
            point.dot(plane.normal) - plane.dist
        };
        num = if d < 0.0 {
            node.children[1]
        } else {
            node.children[0]
        };
    }
    num
}

pub fn recursive_hull_check(
    hull: &Hull<'_>,
    num: i32,
    p1f: f32,
    p2f: f32,
    p1: Vec3,
    p2: Vec3,
    trace: &mut Trace,
) -> bool {
    if num < 0 {
        if num != CONTENTS_SOLID {
            trace.allsolid = false;
            if num == CONTENTS_EMPTY {
                trace.inopen = true;
            } else {
                trace.inwater = true;
            }
        } else {
            trace.startsolid = true;
        }
        return true;
    }

    if num < hull.firstclipnode || num > hull.lastclipnode {
        return false;
    }

    let node = &hull.clipnodes[num as usize];
    let plane = &hull.planes[node.planenum as usize];

    let (t1, t2) = if plane.plane_type < 3 {
        let p1_axis = match plane.plane_type {
            0 => p1.x,
            1 => p1.y,
            _ => p1.z,
        };
        let p2_axis = match plane.plane_type {
            0 => p2.x,
            1 => p2.y,
            _ => p2.z,
        };
        (p1_axis - plane.dist, p2_axis - plane.dist)
    } else {
        (
            p1.dot(plane.normal) - plane.dist,
            p2.dot(plane.normal) - plane.dist,
        )
    };

    if t1 >= 0.0 && t2 >= 0.0 {
        return recursive_hull_check(hull, node.children[0], p1f, p2f, p1, p2, trace);
    }
    if t1 < 0.0 && t2 < 0.0 {
        return recursive_hull_check(hull, node.children[1], p1f, p2f, p1, p2, trace);
    }

    let mut frac = if t1 < 0.0 {
        (t1 + DIST_EPSILON) / (t1 - t2)
    } else {
        (t1 - DIST_EPSILON) / (t1 - t2)
    };
    frac = frac.clamp(0.0, 1.0);

    let mut midf = p1f + (p2f - p1f) * frac;
    let mut mid = Vec3::new(
        p1.x + frac * (p2.x - p1.x),
        p1.y + frac * (p2.y - p1.y),
        p1.z + frac * (p2.z - p1.z),
    );

    let side = t1 < 0.0;

    let near_child = if side {
        node.children[1]
    } else {
        node.children[0]
    };
    let far_child = if side {
        node.children[0]
    } else {
        node.children[1]
    };

    if !recursive_hull_check(hull, near_child, p1f, midf, p1, mid, trace) {
        return false;
    }

    if hull_point_contents(hull, far_child, mid) != CONTENTS_SOLID {
        return recursive_hull_check(hull, far_child, midf, p2f, mid, p2, trace);
    }

    if trace.allsolid {
        return false;
    }

    if !side {
        trace.plane = *plane;
    } else {
        trace.plane = Plane {
            normal: Vec3::new(-plane.normal.x, -plane.normal.y, -plane.normal.z),
            dist: -plane.dist,
            plane_type: plane.plane_type,
            signbits: plane.signbits,
        };
    }

    while hull_point_contents(hull, hull.firstclipnode, mid) == CONTENTS_SOLID {
        frac -= 0.1;
        if frac < 0.0 {
            trace.fraction = midf;
            trace.endpos = mid;
            return false;
        }
        midf = p1f + (p2f - p1f) * frac;
        mid = Vec3::new(
            p1.x + frac * (p2.x - p1.x),
            p1.y + frac * (p2.y - p1.y),
            p1.z + frac * (p2.z - p1.z),
        );
    }

    trace.fraction = midf;
    trace.endpos = mid;
    false
}

pub fn trace_hull(hull: &Hull<'_>, start: Vec3, end: Vec3) -> Trace {
    let mut trace = Trace {
        allsolid: true,
        endpos: end,
        ..Trace::default()
    };

    recursive_hull_check(hull, hull.firstclipnode, 0.0, 1.0, start, end, &mut trace);

    if trace.allsolid {
        trace.startsolid = true;
    }
    if trace.startsolid {
        trace.fraction = 0.0;
    }

    trace
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn box_hull_contents() {
        let box_hull = BoxHull::new(Vec3::new(-1.0, -1.0, -1.0), Vec3::new(1.0, 1.0, 1.0));
        let hull = box_hull.hull();
        assert_eq!(
            hull_point_contents(&hull, hull.firstclipnode, Vec3::new(0.0, 0.0, 0.0)),
            CONTENTS_SOLID
        );
        assert_eq!(
            hull_point_contents(&hull, hull.firstclipnode, Vec3::new(2.0, 0.0, 0.0)),
            CONTENTS_EMPTY
        );
    }

    #[test]
    fn trace_hits_box() {
        let box_hull = BoxHull::new(Vec3::new(-1.0, -1.0, -1.0), Vec3::new(1.0, 1.0, 1.0));
        let hull = box_hull.hull();
        let trace = trace_hull(&hull, Vec3::new(2.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 0.0));
        assert!(trace.fraction < 1.0);
        assert!(trace.endpos.x >= 1.0);
    }
}
