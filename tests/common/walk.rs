use crate::astral_layout::ShipLayout;
use glam::DVec3;
use std::collections::{HashSet, VecDeque};

pub fn key(p:DVec3)->(i32,i32,i32) {(p.x.round() as i32,(p.y*2.0).round() as i32,p.z.round() as i32)}
pub fn step_clear(l:&ShipLayout,a:DVec3,b:DVec3)->bool {
    let y=a.y.max(b.y);
    for i in 0..=4 {
        let t=i as f64/4.0;
        if !l.capsule_clear(DVec3::new(a.x,a.y+(y-a.y)*t,a.z),0.3,1.8) {return false}
        if !l.capsule_clear(DVec3::new(a.x+(b.x-a.x)*t,y,a.z+(b.z-a.z)*t),0.3,1.8) {return false}
        if !l.capsule_clear(DVec3::new(b.x,y+(b.y-y)*t,b.z),0.3,1.8) {return false}
    }
    true
}
pub fn reachable(l:&ShipLayout,from:DVec3,to:DVec3)->bool {
    let mut q=VecDeque::from([from]);
    let mut visited=HashSet::from([key(from)]);
    while let Some(a)=q.pop_front() {
        if (a-to).length()<0.1 {return true}
        if visited.len()>80_000 {return false}
        for (dx,dz) in [(1.0,0.0),(-1.0,0.0),(0.0,1.0),(0.0,-1.0)] {
            let x=a.x+dx; let z=a.z+dz;
            if x.abs()>48.0 || z.abs()>80.0 {continue}
            let Some(y)=l.floor_height(x,z,a.y+0.6) else {continue};
            if y<a.y-0.6 {continue}
            let b=DVec3::new(x,y,z);
            if l.capsule_clear(b,0.3,1.8) && step_clear(l,a,b) && visited.insert(key(b)) {q.push_back(b)}
        }
    }
    false
}
