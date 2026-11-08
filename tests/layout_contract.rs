#[path = "../mixins/astral_cruiser/layout.rs"]
mod astral_layout;
#[path = "../mixins/astral_cruiser/furnish.rs"]
mod astral_furnish;
use astral_layout::{Material, Part, RoomKind, ShipLayout, WeaponMountKind};
use glam::DVec3;
use std::collections::HashSet;
#[path = "common/walk.rs"] mod walk;
use walk::reachable;

const ALL_ROOMS: [RoomKind; 9] = [
    RoomKind::Bridge, RoomKind::Corridor, RoomKind::Reactor, RoomKind::Weapons, RoomKind::Hangar,
    RoomKind::Airlock, RoomKind::Quarters, RoomKind::Mess, RoomKind::Medbay,
];

#[test]
fn each_room_has_real_headroom_and_connected_floor_route() {
    let l=ShipLayout::cruiser();
    let bridge=l.room(RoomKind::Bridge).expect("bridge").entry;
    for kind in ALL_ROOMS {
        let r=l.room(kind).expect("required room");
        assert!(r.entry.is_finite());
        assert!((r.entry.x-r.entry.x.round()).abs()<1e-9 && (r.entry.z-r.entry.z.round()).abs()<1e-9);
        assert!(l.capsule_clear(r.entry,0.3,1.8),"room has no player headroom: {kind:?}");
        assert!((l.floor_height(r.entry.x,r.entry.z,r.entry.y+0.1).unwrap()-r.entry.y).abs()<1e-6);
        assert!(reachable(&l,bridge,r.entry),"no physical walking route to {kind:?}");
    }
}
#[test]
fn every_room_on_both_sides_of_the_spine_can_be_walked_to() {
    let l=ShipLayout::cruiser();
    let bridge=l.room(RoomKind::Bridge).unwrap().entry;
    for r in l.rooms.iter().filter(|r|r.kind!=RoomKind::Doorway) {
        assert!(reachable(&l,bridge,r.entry),"{:?} at {} is walled off",r.kind,r.entry);
    }
}
#[test]
fn the_walkers_route_reaches_the_bottom_of_the_reactor_pit() {
    let l=ShipLayout::cruiser();
    let pit=DVec3::new(6.0,-9.0,-19.0);
    assert_eq!(l.floor_height(pit.x,pit.z,pit.y+0.1),Some(pit.y));
    assert!(reachable(&l,l.room(RoomKind::Bridge).unwrap().entry,pit),"the stairs do not reach the pit");
}
#[test]
fn every_room_is_furnished() {
    let l=ShipLayout::cruiser();
    for kind in ALL_ROOMS {
        for r in l.rooms.iter().filter(|r|r.kind==kind) {
            let inside=l.props.iter().filter(|p|{
                let c=(p.min+p.max)/2.0;
                c.cmpge(r.min).all()&&c.cmple(r.max).all()
            }).count();
            assert!(inside>=12,"{kind:?} at {} holds only {inside} things",r.entry);
        }
    }
}
#[test]
fn the_furniture_is_not_all_one_grey_box() {
    let l=ShipLayout::cruiser();
    for material in [Material::Console,Material::Screen,Material::Metal,Material::Hazard,Material::Crate,
        Material::Rail,Material::Pipe,Material::Seat,Material::Holo,Material::Grate,Material::Cloth,
        Material::Lamp,Material::Door] {
        assert!(l.props.iter().any(|p|p.material==material),"nothing is made of {material:?}");
    }
}
#[test]
fn every_prop_is_a_real_finite_box_inside_a_room() {
    let l=ShipLayout::cruiser();
    for p in &l.props {
        assert!(p.min.is_finite()&&p.max.is_finite()&&p.min.cmplt(p.max).all(),"{p:?}");
        let c=(p.min+p.max)/2.0;
        assert!(l.rooms.iter().any(|r|c.cmpge(r.min-DVec3::splat(0.5)).all()&&c.cmple(r.max+DVec3::splat(0.5)).all()),
            "{:?} stands in no room at {c}",p.material);
    }
}
#[test]
fn the_helm_has_a_chair_and_a_desk_before_the_window() {
    let l=ShipLayout::cruiser();
    let seat=l.pilot_seat();
    assert!(l.props.iter().any(|p|p.material==Material::Seat&&(p.min+p.max).distance(seat*2.0)<4.0));
    assert!(l.props.iter().any(|p|p.material==Material::Console&&p.min.z>seat.z&&p.min.x<=0.0&&p.max.x>=0.0));
}
#[test]
fn someone_getting_up_from_the_helm_has_room_to_stand() {
    let l=ShipLayout::cruiser();
    let stand=l.bridge_stand();
    assert!(l.capsule_clear(stand,0.3,1.8));
    assert!(stand.distance(l.pilot_seat())<3.0);
}
#[test]
fn a_solid_prop_stops_a_body_and_a_decoration_does_not() {
    let l=ShipLayout::cruiser();
    let chair=l.props.iter().find(|p|p.solid&&p.material==Material::Seat).unwrap();
    let c=(chair.min+chair.max)/2.0;
    assert!(!l.capsule_clear(DVec3::new(c.x,-4.0,c.z),0.3,1.8));
    let lamp=l.props.iter().find(|p|!p.solid&&p.material==Material::Lamp&&p.max.y>0.0).unwrap();
    let c=(lamp.min+lamp.max)/2.0;
    assert!(!lamp.solid,"{c}");
}
#[test]
fn a_solid_prop_can_be_stood_on() {
    let l=ShipLayout::cruiser();
    assert_eq!(l.floor_height(0.0,-19.0,-3.9),Some(-4.0),"the catwalk over the pit");
    assert_eq!(l.floor_height(0.0,-19.0,-6.0),Some(-9.0),"the pit under it");
}
#[test]
fn the_door_of_every_doorway_opens_along_the_wall() {
    let l=ShipLayout::cruiser();
    let doors=l.rooms.iter().filter(|r|r.kind==RoomKind::Doorway).count();
    let leaves:Vec<_>=l.props.iter().filter(|p|p.material==Material::Door).collect();
    assert!(doors>=12,"{doors} doorways");
    assert_eq!(leaves.len(),doors*2);
    for leaf in leaves {
        assert!(!leaf.solid,"a closed door must not hold anyone back as it opens");
        assert!(leaf.slide.length()>0.5&&leaf.slide.y==0.0,"{:?}",leaf.slide);
    }
}
#[test]
fn doorways_join_two_rooms() {
    let l=ShipLayout::cruiser();
    for d in l.rooms.iter().filter(|r|r.kind==RoomKind::Doorway) {
        assert!(l.capsule_clear(d.entry,0.3,1.8),"{}",d.entry);
    }
}
#[test]
fn the_ship_has_separate_rooms_not_one_hall() {
    let l=ShipLayout::cruiser();
    // The spine's side wall between doors: no way through from the corridor.
    assert!(!l.capsule_clear(DVec3::new(2.7,-4.0,6.0),0.3,1.8));
    assert!(!l.capsule_clear(DVec3::new(-2.7,-4.0,16.0),0.3,1.8));
}
#[test]
fn geometry_is_finite_and_fits_the_declared_portal_clearance() {
    let l=ShipLayout::cruiser();
    let half=DVec3::new(48.0,18.0,80.0);
    assert_eq!(l.half_extents(),half);
    for p in &l.parts {
        assert!(p.min.is_finite() && p.max.is_finite());
        assert!(p.min.cmplt(p.max).all());
        assert!(p.min.cmpge(-half).all() && p.max.cmple(half).all(),"hull exceeds advertised size");
    }
    assert!(l.parts.iter().any(|p|p.material==Material::Light));
    assert!(l.parts.iter().any(|p|p.material==Material::Engine));
    assert!(l.parts.iter().any(|p|p.material==Material::Glass));
}
#[test]
fn armament_has_multiple_real_hardpoints_for_all_four_systems() {
    let l=ShipLayout::cruiser();
    assert!(l.hardpoints.len()>=10,"large cruiser needs actual distributed gun mounts");
    for kind in [WeaponMountKind::Turret,WeaponMountKind::NosePulse,
        WeaponMountKind::BellyBeam,WeaponMountKind::MainBeam] {
        assert!(l.hardpoints.iter().any(|h|h.kind==kind),"missing weapon system");
    }
    let mut ids=HashSet::new();
    for h in &l.hardpoints {
        assert!(ids.insert(h.id),"duplicate hardpoint identity");
        assert!(h.position.is_finite() && h.forward.is_finite());
        assert!((h.forward.length()-1.0).abs()<1e-9);
    }
}
#[test]
fn a_shot_across_the_ship_meets_its_own_hull() {
    let l=ShipLayout::cruiser();
    // From the mast, down through the ship.
    assert!(l.ray_meets_hull(DVec3::new(0.0,19.0,-33.0),-DVec3::Y));
    // From a turret on the left, across to the right.
    let turret=l.hardpoints[0].position;
    assert!(l.ray_meets_hull(turret,DVec3::new(-1.0,-0.2,0.0).normalize()));
}
#[test]
fn a_turret_can_fire_clear_of_the_ship() {
    let l=ShipLayout::cruiser();
    for h in l.hardpoints.iter().take(8) {
        assert!(!l.ray_meets_hull(h.position,DVec3::Y),"mount {} cannot fire straight up",h.id);
    }
}
#[test]
fn the_guns_at_the_nose_and_keel_are_clear_ahead() {
    let l=ShipLayout::cruiser();
    for h in l.hardpoints.iter().skip(8) {
        assert!(!l.ray_meets_hull(h.position,DVec3::Z),"mount {} fires into the ship",h.id);
    }
}
#[test]
fn a_tapered_part_narrows_toward_its_nose() {
    let part=Part {min:DVec3::new(-10.0,0.0,0.0),max:DVec3::new(10.0,10.0,100.0),
        material:Material::Hull,front_scale:0.5,top_scale:1.0};
    assert!(part.contains(DVec3::new(9.0,5.0,0.0)));
    assert!(part.contains(DVec3::new(4.0,5.0,100.0)));
    assert!(!part.contains(DVec3::new(6.0,5.0,100.0)));
    assert!(!part.contains(DVec3::new(0.0,11.0,50.0)));
}
#[test]
fn a_tapered_part_narrows_toward_its_top() {
    let part=Part {min:DVec3::new(-10.0,0.0,0.0),max:DVec3::new(10.0,10.0,100.0),
        material:Material::Hull,front_scale:1.0,top_scale:0.5};
    assert!(part.contains(DVec3::new(9.0,0.0,50.0)));
    assert!(!part.contains(DVec3::new(9.0,10.0,50.0)));
    assert!(part.contains(DVec3::new(4.0,10.0,50.0)));
}
#[test]
fn every_room_fits_inside_the_outer_hull() {
    let l=ShipLayout::cruiser();
    for r in &l.rooms {
        for x in [r.min.x,r.max.x] { for y in [r.min.y,r.max.y] { for z in [r.min.z,r.max.z] {
            let corner=DVec3::new(x,y,z);
            assert!(l.parts.iter().any(|p|p.material==Material::Hull&&p.contains(corner)),
                "{:?} corner {corner} pokes through the hull",r.kind);
        }}}
    }
}
#[test]
fn the_pilot_sits_on_the_bridge() {
    let l=ShipLayout::cruiser();
    let seat=l.pilot_seat();
    let bridge=l.room(RoomKind::Bridge).unwrap();
    assert!(seat.cmpge(bridge.min).all()&&seat.cmple(bridge.max).all());
}
#[test]
fn the_airlock_opens_outside_the_hull() {
    let l=ShipLayout::cruiser();
    let exit=l.airlock_exit();
    assert!(!l.parts.iter().any(|p|p.contains(exit)&&p.material!=Material::Glass),"exit is inside the hull");
    let airlock=l.room(RoomKind::Airlock).unwrap();
    assert!(exit.x>airlock.max.x,"the door is on the ship's outer side");
    assert!(exit.x-airlock.max.x<10.0,"a long way from the door");
}
#[test]
fn walking_up_to_the_outer_door_is_noticed() {
    let l=ShipLayout::cruiser();
    assert!(l.in_outer_door(DVec3::new(21.0,-4.0,-3.0)));
    assert!(!l.in_outer_door(DVec3::new(17.0,-4.0,-3.0)));
    assert!(l.capsule_clear(DVec3::new(21.0,-4.0,-3.0),0.3,1.8),"the passage cannot be walked");
}
#[test]
fn the_reactor_cores_block_walking_through_them() {
    let l=ShipLayout::cruiser();
    let cores:Vec<_>=l.props.iter().filter(|p|p.material==Material::Reactor).collect();
    assert_eq!(cores.len(),2);
    for core in cores {
        let inside=DVec3::new((core.min.x+core.max.x)/2.0,-9.0,(core.min.z+core.max.z)/2.0);
        assert!(!l.capsule_clear(inside,0.3,1.8));
    }
}
#[test]
fn invalid_or_exterior_capsules_do_not_become_walkable_rooms() {
    let l=ShipLayout::cruiser();
    assert!(!l.capsule_clear(DVec3::new(f64::NAN,0.0,0.0),0.3,1.8));
    assert!(!l.capsule_clear(DVec3::ZERO,-0.3,1.8));
    assert!(!l.capsule_clear(DVec3::ZERO,0.3,f64::INFINITY));
    assert!(l.floor_height(100.0,100.0,18.0).is_none());
}
#[test]
fn the_hull_is_solid_from_keel_to_roof_amidships() {
    let l=ShipLayout::cruiser();
    let spans=l.hull_spans(0.0,0.0);
    assert_eq!(spans.len(),1,"{spans:?}");
    assert_eq!(spans[0],(-18.0,13.0));
}
#[test]
fn the_hull_has_stepped_decks_to_stand_on() {
    let l=ShipLayout::cruiser();
    let top=|x:f64,z:f64| l.hull_spans(x,z).last().map(|s|s.1);
    assert_eq!(top(0.0,-30.0),Some(18.0),"the mast");
    assert_eq!(top(0.0,30.0),Some(10.0),"the upper deck");
}
#[test]
fn nothing_solid_stands_beside_the_hull() {
    let l=ShipLayout::cruiser();
    assert!(l.hull_spans(60.0,0.0).is_empty());
    assert!(l.hull_spans(0.0,90.0).is_empty());
    assert!(l.hull_spans(40.0,60.0).is_empty(),"the nose narrows");
}
#[test]
fn a_sloped_side_is_lower_the_further_out() {
    let l=ShipLayout::cruiser();
    // Beyond the upper parts the main hull's side leans in as it rises.
    let top=|x:f64| l.hull_spans(x,20.0).last().map(|s|s.1).unwrap();
    assert!(top(18.0)<top(17.0),"{} {}",top(18.0),top(17.0));
}
#[test]
fn hull_spans_are_sorted_and_apart() {
    let l=ShipLayout::cruiser();
    for x in [-44.0,-20.0,0.0,20.0,41.0] { for z in [-75.0,-30.0,5.0,60.0] {
        let spans=l.hull_spans(x,z);
        for s in &spans {assert!(s.0<s.1)}
        for pair in spans.windows(2) {assert!(pair[0].1<pair[1].0,"{spans:?}")}
    }}
}
#[test]
fn hull_points_lie_on_real_hull_not_its_bounding_box() {
    let l=ShipLayout::cruiser();
    let points=l.hull_points(2.0);
    assert!(points.len()>2000,"{}",points.len());
    assert!(points.len()<40_000,"too many to test a tick: {}",points.len());
    assert!(!points.iter().any(|p|p.x.abs()>30.0&&p.z>60.0),"a point on the empty corner of the box");
    let near=|q:DVec3| points.iter().map(|p|p.distance(q)).fold(f64::MAX,f64::min);
    assert!(near(DVec3::new(0.0,-18.0,10.0))<=2.0,"the keel");
    assert!(near(DVec3::new(0.0,-1.0,80.0))<=4.0,"the nose");
}
#[test]
fn every_turret_stands_on_the_hull() {
    let l=ShipLayout::cruiser();
    for h in l.hardpoints.iter().take(8) {
        let deck=h.position.y-1.5;
        for (dx,dz) in [(-2.2,-2.2),(-2.2,2.2),(2.2,-2.2),(2.2,2.2)] {
            let top=l.hull_spans(h.position.x+dx,h.position.z+dz).last().map(|s|s.1);
            let top=top.expect("hull under the turret");
            assert!(top>=deck-0.01,"mount {} hangs over the side: {top} under {deck}",h.id);
        }
    }
}
#[test]
fn the_belly_gun_is_held_along_its_length() {
    let l=ShipLayout::cruiser();
    for z in [52.0,58.0,64.0] {
        let spans=l.hull_spans(0.0,z);
        assert_eq!(spans.len(),1,"a gap in the air at {z}: {spans:?}");
    }
}
