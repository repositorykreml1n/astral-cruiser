// Everything standing in the rooms, room by room. A prop is solid when a walker
// should bump into it; rails, pipes and lamps are only seen, and a rail's stop is
// a hidden barrier so the eye passes between its bars.
use super::astral_layout::{Face, Material, Material::*, Prop, Room, RoomKind, Shape};
use glam::DVec3;

const DECK: f64 = -4.0;

fn make(min: [f64; 3], max: [f64; 3], material: Material, shape: Shape, solid: bool) -> Prop {
    Prop {
        min: DVec3::from_array(min),
        max: DVec3::from_array(max),
        material,
        shape,
        solid,
        slide: DVec3::ZERO,
        door: DVec3::ZERO,
    }
}

fn solid(min: [f64; 3], max: [f64; 3], material: Material) -> Prop {
    make(min, max, material, Shape::Block, true)
}

fn decor(min: [f64; 3], max: [f64; 3], material: Material) -> Prop {
    make(min, max, material, Shape::Block, false)
}

fn barrier(min: [f64; 3], max: [f64; 3]) -> Prop {
    make(min, max, Metal, Shape::Hidden, true)
}

fn pillar(x: f64, z: f64, radius: f64, y: [f64; 2], material: Material) -> Prop {
    let min = [x - radius, y[0], z - radius];
    make(min, [x + radius, y[1], z + radius], material, Shape::Column, true)
}

fn glow(x: f64, z: f64, radius: f64, y: [f64; 2], material: Material) -> Prop {
    let min = [x - radius, y[0], z - radius];
    make(min, [x + radius, y[1], z + radius], material, Shape::Column, false)
}

/// A desk whose working top leans toward `face`.
fn desk(min: [f64; 3], max: [f64; 3], face: Face, low: f64) -> Prop {
    make(min, max, Console, Shape::Slope { face, low }, true)
}

/// These props and their twins on the other side of the ship.
fn twinned(props: Vec<Prop>) -> Vec<Prop> {
    let twins: Vec<Prop> = props.iter().map(|p| p.mirrored()).collect();
    props.into_iter().chain(twins).collect()
}

/// A chair facing +Z, its seat's middle at (x, z).
fn chair(x: f64, z: f64) -> Vec<Prop> {
    vec![
        pillar(x, z, 0.3, [DECK, -3.55], Metal),
        solid([x - 0.5, -3.55, z - 0.5], [x + 0.5, -3.25, z + 0.5], Seat),
        solid([x - 0.5, -3.25, z - 0.65], [x + 0.5, -1.9, z - 0.45], Seat),
        decor([x - 0.28, -1.9, z - 0.65], [x + 0.28, -1.5, z - 0.45], Seat),
        decor([x - 0.68, -3.0, z - 0.5], [x - 0.5, -2.88, z + 0.4], Rail),
        decor([x + 0.5, -3.0, z - 0.5], [x + 0.68, -2.88, z + 0.4], Rail),
    ]
}

fn bridge() -> Vec<Prop> {
    let mut props = chair(0.0, 50.0);
    // The helm: the desk wraps round the pilot's chair.
    props.extend([
        desk([-3.2, DECK, 51.2], [3.2, -2.7, 52.8], Face::Back, -3.3),
        decor([-3.2, -2.7, 52.5], [3.2, -2.1, 52.8], Screen),
        desk([3.2, DECK, 49.8], [5.4, -2.7, 52.2], Face::Back, -3.3),
        desk([-5.4, DECK, 49.8], [-3.2, -2.7, 52.2], Face::Back, -3.3),
        decor([-1.2, -3.22, 51.6], [1.2, -3.1, 52.0], Lamp),
        // The window's frame.
        decor([-8.0, -2.6, 53.7], [8.0, -2.4, 54.0], Trim),
        decor([-8.0, 1.4, 53.7], [8.0, 1.6, 54.0], Trim),
        decor([-0.12, -2.5, 53.7], [0.12, 1.5, 54.0], Trim),
        // The chart table at the back of the bridge, with the ship in light above it.
        pillar(0.0, 43.5, 1.7, [DECK, -3.0], Console),
        glow(0.0, 43.5, 1.75, [-3.0, -2.92], Screen),
        glow(0.0, 43.5, 1.45, [-2.92, -1.3], Holo),
        decor([-3.0, -2.1, 40.02], [3.0, 0.8, 40.12], Screen),
    ]);
    let mut side = vec![
        // Stations along the bridge's walls, each with its chair and displays.
        desk([7.2, DECK, 41.5], [8.9, -2.7, 45.5], Face::Right, -3.3),
        decor([8.8, -2.6, 41.5], [8.95, -0.3, 45.5], Screen),
        decor([5.0, 1.72, 41.0], [5.5, 1.98, 53.0], Lamp),
        decor([3.1, -2.6, 53.7], [3.34, 1.5, 54.0], Trim),
        decor([6.2, -2.6, 53.7], [6.44, 1.5, 54.0], Trim),
        decor([8.6, -2.0, 49.2], [8.95, 1.2, 49.5], Trim),
        decor([3.2, -2.1, 40.02], [8.0, -0.2, 40.12], Screen),
        decor([7.0, -4.0, 40.02], [8.95, 2.0, 40.3], Metal),
        solid([8.3, DECK, 46.0], [9.0, -3.2, 53.6], Metal),
        decor([8.3, -3.2, 46.0], [8.36, -3.1, 53.6], Lamp),
    ];
    let seats = [chair(6.0, 43.5)];
    for mut seat in seats {
        for prop in &mut seat {
            // Turned toward the wall's desk: the back on the room's side.
            if prop.material == Seat && prop.max.z - prop.min.z < 0.4 {
                prop.min = DVec3::new(5.35, prop.min.y, 43.0);
                prop.max = DVec3::new(5.55, prop.max.y, 44.0);
            }
        }
        side.extend(seat);
    }
    props.extend(twinned(side));
    props
}

fn corridors() -> Vec<Prop> {
    let mut props = Vec::new();
    // The way the spine runs: from the bridge's door to the hall, and behind it.
    for (from, to) in [(-7.5, 39.5), (-42.0, -30.5)] {
        props.push(decor([-0.08, DECK, from], [0.08, -3.97, to], Lamp));
        let sides = vec![
            decor([1.95, 1.05, from], [2.22, 1.32, to], Pipe),
            decor([1.95, 1.45, from], [2.22, 1.72, to], Pipe),
            decor([2.3, -4.0, from], [2.5, -3.72, to], Metal),
            decor([1.7, DECK, from], [1.75, -3.97, to], Hazard),
        ];
        props.extend(twinned(sides));
        let ribs = ((from / 6.0).ceil() as i32)..=((to / 6.0).floor() as i32);
        for rib in ribs {
            let z = f64::from(rib) * 6.0;
            props.push(decor([-2.5, 1.68, z - 0.25], [2.5, 2.0, z + 0.25], Metal));
            props.extend(twinned(vec![
                decor([2.32, DECK, z - 0.25], [2.5, 2.0, z + 0.25], Metal),
                decor([2.26, -2.0, z - 0.1], [2.32, -1.4, z + 0.1], Lamp),
            ]));
        }
    }
    // The side passage to the airlock.
    props.push(decor([3.0, -0.04 + DECK, -3.58], [14.0, DECK + 0.03, -3.42], Lamp));
    let along = [
        decor([3.0, 1.45, -4.9], [14.0, 1.72, -4.63], Pipe),
        decor([3.0, 1.05, -4.9], [14.0, 1.32, -4.63], Pipe),
        decor([3.0, 1.45, -2.37], [14.0, 1.72, -2.1], Pipe),
        decor([3.0, DECK, -5.0], [14.0, -3.72, -4.8], Metal),
        decor([3.0, DECK, -2.2], [14.0, -3.72, -2.0], Metal),
    ];
    props.extend(along);
    for x in [5.0, 8.5, 12.0] {
        props.extend([
            decor([x - 0.25, DECK, -5.0], [x + 0.25, 2.0, -4.82], Metal),
            decor([x - 0.25, DECK, -2.18], [x + 0.25, 2.0, -2.0], Metal),
            decor([x - 0.25, 1.68, -5.0], [x + 0.25, 2.0, -2.0], Metal),
            decor([x - 0.1, -2.0, -4.84], [x + 0.1, -1.4, -4.78], Lamp),
        ]);
    }
    // Lockers and readouts between the ribs, facing the walkway.
    for z in [15.0, 21.0, -3.0 + 3.0 * 3.0, -39.0, -33.0] {
        props.extend(twinned(vec![
            decor([2.4, -3.72, z], [2.5, 0.2, z + 2.0], Metal),
            decor([2.36, -1.2, z + 0.4], [2.4, 0.0, z + 1.6], Screen),
        ]));
    }
    props
}

fn quarters() -> Vec<Prop> {
    let mut props = vec![
        // The desk under the end wall and its lamp.
        solid([3.6, DECK, 36.6], [8.0, -3.1, 38.0], Metal),
        decor([4.4, -3.1, 37.5], [6.4, -2.0, 37.6], Screen),
        glow(7.3, 37.3, 0.12, [-3.1, -2.5], Rail),
        glow(7.3, 37.3, 0.3, [-2.5, -2.3], Lamp),
        pillar(5.4, 35.7, 0.32, [DECK, -3.5], Seat),
        // Rug and ceiling lamps.
        decor([3.6, DECK, 27.0], [8.8, -3.98, 35.5], Cloth),
        decor([5.0, 1.8, 26.0], [10.0, 1.98, 26.3], Lamp),
        decor([5.0, 1.8, 34.0], [10.0, 1.98, 34.3], Lamp),
    ];
    // Lockers along the wall by the door.
    for i in 0..5 {
        let z = 24.1 + 1.0 * f64::from(i);
        props.push(solid([3.0, DECK, z], [3.7, 1.0, z + 0.9], Metal));
        props.push(decor([3.7, -1.4, z + 0.1], [3.72, -1.0, z + 0.3], Lamp));
    }
    // Bunks in two tiers along the outer wall, under the windows' ends.
    for z in [25.0, 29.4, 33.8] {
        props.extend([
            solid([9.4, DECK, z], [11.9, -3.5, z + 1.9], Metal),
            solid([9.4, -3.5, z + 0.03], [11.87, -3.2, z + 1.87], Cloth),
            decor([11.3, -3.2, z + 0.3], [11.85, -3.05, z + 1.6], Cloth),
            solid([9.4, -2.1, z], [11.9, -1.8, z + 1.9], Metal),
            decor([9.4, -1.8, z + 0.03], [11.87, -1.55, z + 1.87], Cloth),
            solid([9.3, DECK, z], [9.42, -1.5, z + 0.1], Rail),
            solid([9.3, DECK, z + 1.8], [9.42, -1.5, z + 1.9], Rail),
            decor([9.42, -1.72, z + 0.1], [9.5, -1.58, z + 1.8], Rail),
            decor([11.86, -3.0, z + 0.2], [11.9, -2.3, z + 1.7], Screen),
        ]);
    }
    props
}

fn mess() -> Vec<Prop> {
    let mut props = vec![
        // The galley: a counter, hob, and the food dispenser's lit menu.
        solid([-12.0, DECK, 36.4], [-4.0, -3.0, 38.0], Metal),
        decor([-12.0, -3.0, 36.4], [-4.0, -2.92, 38.0], Rail),
        solid([-11.6, -2.92, 37.3], [-9.0, 0.4, 38.0], Metal),
        decor([-11.3, -1.6, 37.26], [-9.3, -0.1, 37.3], Screen),
        decor([-8.6, -2.9, 36.7], [-7.4, -2.86, 37.7], Lamp),
        decor([-6.8, 0.2, 37.2], [-4.0, 1.9, 38.0], Metal),
        decor([-6.6, -0.2, 37.5], [-4.2, 0.2, 37.8], Lamp),
        decor([-4.0, -4.0, 24.0], [-3.0, 2.0, 24.3], Metal),
        decor([-10.0, 1.8, 28.8], [-5.0, 1.98, 29.1], Lamp),
        decor([-10.0, 1.8, 32.8], [-5.0, 1.98, 33.1], Lamp),
        decor([-8.4, 1.0, 24.02], [-4.6, 0.6 + 1.0, 24.1], Screen),
    ];
    // Tables with a bench down each side.
    for z in [25.2, 33.0] {
        props.extend([
            solid([-11.2, DECK, z + 1.05], [-5.4, -3.08, z + 2.05], Metal),
            decor([-11.25, -3.08, z + 1.0], [-5.35, -3.0, z + 2.1], Rail),
            solid([-11.0, DECK, z + 0.1], [-5.6, -3.5, z + 0.6], Seat),
            solid([-11.0, DECK, z + 2.5], [-5.6, -3.5, z + 3.0], Seat),
            decor([-8.5, -3.0, z + 1.4], [-8.2, -2.84, z + 1.7], Lamp),
        ]);
        // A place laid at each seat: a tray and a mug.
        for x in [-10.4, -9.0, -7.6, -6.2] {
            props.extend([
                decor([x - 0.25, -3.0, z + 1.12], [x + 0.25, -2.97, z + 1.42], Rail),
                decor([x - 0.25, -3.0, z + 1.73], [x + 0.25, -2.97, z + 2.03], Rail),
                glow(x + 0.36, z + 1.3, 0.06, [-3.0, -2.86], Cloth),
                glow(x - 0.36, z + 1.85, 0.06, [-3.0, -2.86], Cloth),
            ]);
        }
    }
    props
}

/// One side's gun bay: the guns' breeches overhead and the shells racked for them.
fn gun_bay() -> Vec<Prop> {
    let mut props = vec![
        // Fire control at the bay's far end, clear of the aisle by the door.
        desk([5.5, DECK, 18.2], [12.5, -2.7, 19.8], Face::Back, -3.3),
        decor([5.5, -2.7, 19.5], [12.5, -1.0, 19.8], Screen),
        decor([3.1, -2.2, 19.9], [5.0, 0.6, 19.98], Screen),
        decor([15.9, -2.2, 9.0], [15.98, 0.6, 13.0], Screen),
        decor([4.0, -3.98, 8.5], [15.0, -3.96, 8.65], Hazard),
        decor([4.0, -3.98, 13.35], [15.0, -3.96, 13.5], Hazard),
    ];
    props.extend(chair(9.0, 16.6));
    // Three breeches across the bay, each a drum on a pedestal with its feed.
    for x in [6.0, 10.0, 14.0] {
        props.extend([
            pillar(x, 5.5, 1.2, [DECK, -1.0], Metal),
            glow(x, 5.5, 1.35, [-2.6, -2.3], Hazard),
            glow(x, 5.5, 0.95, [-1.0, 0.4], Pipe),
            decor([x - 0.4, 0.4, 5.1], [x + 0.4, 2.0, 5.9], Pipe),
            decor([x - 0.6, 1.5, 2.1], [x + 0.6, 2.0, 15.8], Metal),
            decor([x - 0.15, -1.5, 4.25], [x + 0.15, -1.2, 4.3], Lamp),
        ]);
        // A rack of shells on the aisle's far side.
        for row in 0..3 {
            let y = -3.7 + 1.2 * f64::from(row);
            props.push(solid([x - 1.45, y - 0.3, 14.2], [x + 1.45, y - 0.2, 15.6], Metal));
            for shell in 0..4 {
                let along = x - 1.05 + 0.7 * f64::from(shell);
                props.push(glow(along, 14.9, 0.3, [y - 0.2, y + 0.65], Crate));
            }
        }
        props.push(solid([x - 1.5, DECK, 14.15], [x - 1.42, 0.0, 15.65], Rail));
        props.push(solid([x + 1.42, DECK, 14.15], [x + 1.5, 0.0, 15.65], Rail));
    }
    props
}

fn sickbay() -> Vec<Prop> {
    let mut props = vec![
        // The scanner's arch over the middle bed.
        decor([-12.6, DECK, -6.0], [-12.3, -0.6, -3.8], Rail),
        decor([-9.7, DECK, -6.0], [-9.4, -0.6, -3.8], Rail),
        decor([-12.6, -0.9, -5.2], [-9.4, -0.6, -4.6], Lamp),
        decor([-12.3, -2.6, -5.1], [-9.7, -2.5, -4.7], Holo),
        // Cabinets and the doctor's desk.
        desk([-7.0, DECK, 0.0], [-3.2, -2.7, 0.95], Face::Back, -3.3),
        decor([-7.0, -2.7, 0.9], [-3.2, -0.8, 0.98], Screen),
        decor([-18.0, 1.76, -3.2], [-5.0, 1.98, -2.9], Lamp),
        decor([-18.0, 1.76, 0.0], [-5.0, 1.98, 0.3], Lamp),
        decor([-19.9, -2.6, -2.0], [-19.8, -0.2, 0.8], Screen),
    ];
    for x in [-18.0, -12.0, -6.0] {
        props.extend([
            solid([x - 0.9, DECK, -6.95], [x + 0.9, -3.3, -3.3], Metal),
            solid([x - 0.85, -3.3, -6.92], [x + 0.85, -3.05, -3.33], Cloth),
            decor([x - 0.6, -3.05, -6.75], [x + 0.6, -2.9, -6.0], Cloth),
            decor([x - 0.8, -1.8, -6.97], [x + 0.8, -0.4, -6.9], Screen),
            pillar(x + 1.2, -6.2, 0.05, [DECK, -1.4], Rail),
            decor([x + 1.05, -1.5, -6.3], [x + 1.35, -1.1, -6.1], Holo),
        ]);
    }
    for i in 0..6 {
        let x = -19.9 + 1.15 * f64::from(i);
        props.push(solid([x, DECK, 0.1], [x + 1.05, 1.0, 1.0], Cloth));
        props.push(decor([x + 0.45, -1.6, 0.06], [x + 0.6, -1.2, 0.1], Lamp));
    }
    props
}

fn airlock() -> Vec<Prop> {
    let mut props = vec![
        // Hazard stripes round the way out.
        decor([19.94, -0.5, -4.8], [20.0, -0.2, -1.2], Hazard),
        decor([19.94, DECK, -4.9], [20.0, -0.2, -4.6], Hazard),
        decor([19.94, DECK, -1.4], [20.0, -0.2, -1.1], Hazard),
        decor([14.6, -3.98, -5.2], [19.9, -3.96, -5.0], Hazard),
        decor([14.6, -3.98, -2.0], [19.9, -3.96, -1.8], Hazard),
        decor([14.6, -3.99, -5.0], [20.0, -3.97, -2.0], Grate),
        // A bench to pull boots on, and the lit ring of the inner door.
        solid([14.6, DECK, 0.3], [19.8, -3.5, 1.0], Seat),
        decor([14.52, -0.5, -5.3], [14.58, -0.3, -1.7], Lamp),
        decor([15.0, 1.76, -3.7], [19.5, 1.98, -3.3], Lamp),
        decor([19.92, -2.2, -6.6], [20.0, -0.6, -5.2], Screen),
    ];
    // Suits racked along the far wall.
    for i in 0..4 {
        let x = 15.0 + 1.3 * f64::from(i);
        props.extend([
            solid([x, DECK, -7.0], [x + 1.1, 1.0, -6.3], Metal),
            decor([x + 0.25, -2.4, -6.32], [x + 0.85, -0.9, -6.25], Rail),
            decor([x + 0.38, -0.8, -6.3], [x + 0.72, -0.4, -6.26], Screen),
            decor([x + 0.1, -0.2, -6.3], [x + 1.0, -0.1, -6.28], Lamp),
        ]);
    }
    props
}

/// The reactor hall. The spine crosses on a catwalk; below it is the pit, with
/// a core on each side and stairs down.
fn reactor() -> Vec<Prop> {
    const PIT: f64 = -9.0;
    const TOP: f64 = 4.0;
    let mut props = vec![
        solid([-2.5, -4.5, -30.0], [2.5, DECK, -8.0], Grate),
        decor([-0.08, DECK, -30.0], [0.08, -3.97, -8.0], Lamp),
        decor([-20.0, 3.4, -19.4], [20.0, 3.9, -18.6], Metal),
        decor([-20.0, 2.6, -28.0], [20.0, 2.9, -27.7], Pipe),
        decor([-20.0, 2.6, -10.3], [20.0, 2.9, -10.0], Pipe),
    ];
    let mut side = vec![
        // The core, in its ringed sleeve.
        pillar(13.0, -19.0, 4.0, [PIT, TOP], Reactor),
        glow(13.0, -19.0, 5.2, [PIT, -8.0], Metal),
        glow(13.0, -19.0, 4.7, [-1.2, -0.7], Metal),
        glow(13.0, -19.0, 4.4, [-1.3, -1.2], Lamp),
        glow(13.0, -19.0, 4.7, [-5.4, -4.9], Metal),
        glow(13.0, -19.0, 4.4, [-5.5, -5.4], Lamp),
        glow(13.0, -19.0, 5.0, [3.2, 3.8], Metal),
        // Conduits from the core's head to the catwalk's roof.
        decor([2.0, 2.0, -19.5], [9.0, 2.6, -18.5], Pipe),
        decor([2.0, 1.2, -21.5], [9.5, 1.5, -21.2], Pipe),
        decor([2.0, 1.2, -16.8], [9.5, 1.5, -16.5], Pipe),
        // The catwalk's rail, with the top of the stairs left open.
        barrier([2.35, DECK, -30.0], [2.5, -2.7, -13.0]),
        barrier([2.35, DECK, -10.0], [2.5, -2.7, -8.0]),
        decor([2.38, -2.95, -30.0], [2.46, -2.87, -13.0], Rail),
        decor([2.38, -3.5, -30.0], [2.46, -3.44, -13.0], Rail),
        decor([2.38, -2.95, -10.0], [2.46, -2.87, -8.0], Rail),
        decor([2.38, -3.5, -10.0], [2.46, -3.44, -8.0], Rail),
        // The pit's floor markings and desks by the core.
        decor([6.0, PIT, -28.5], [19.5, -8.98, -28.3], Hazard),
        decor([6.0, PIT, -9.7], [19.5, -8.98, -9.5], Hazard),
        desk([6.0, PIT, -28.0], [10.0, -7.7, -26.0], Face::Front, -8.3),
        decor([6.0, -7.7, -28.0], [10.0, -6.0, -27.8], Screen),
        decor([19.9, -7.6, -27.0], [20.0, -3.0, -22.0], Screen),
        decor([19.9, -7.6, -16.0], [20.0, -3.0, -11.0], Screen),
        decor([19.6, PIT, -19.8], [20.0, TOP, -18.2], Pipe),
        decor([19.0, 3.0, -30.0], [19.4, 3.5, -8.0], Pipe),
    ];
    for z in [-27.0, -21.0, -15.0, -9.0] {
        side.push(solid([2.1, PIT, z - 0.2], [2.5, -4.5, z + 0.2], Metal));
    }
    for z in [-29.0, -26.0, -23.0, -20.0, -17.0, -14.0, -9.5] {
        side.push(decor([2.38, DECK, z - 0.04], [2.46, -2.87, z + 0.04], Rail));
    }
    // Stairs down from the gap in the rail: half a block for each block.
    for step in 0..9 {
        let (x, top) = (2.5 + f64::from(step), -4.5 - 0.5 * f64::from(step));
        side.push(solid([x, PIT, -13.0], [x + 1.0, top, -10.0], Grate));
    }
    side.extend([
        decor([2.5, -3.55, -13.04], [12.5, -3.5, -12.98], Hazard),
        barrier([3.0, -8.5, -13.15], [12.0, -2.7, -13.0]),
        barrier([3.0, -8.5, -10.0], [12.0, -2.7, -9.85]),
    ]);
    props.extend(twinned(side));
    props
}

/// A stack of crates: `layers` of them, each a little smaller than the one under.
fn stack(x: f64, z: f64, wide: f64, deep: f64, layers: u32) -> Vec<Prop> {
    (0..layers)
        .map(|layer| {
            let inset = 0.15 * f64::from(layer);
            let y = DECK + 2.0 * f64::from(layer);
            solid([x + inset, y, z + inset], [x + wide - inset, y + 2.0, z + deep - inset], Crate)
        })
        .collect()
}

fn cargo() -> Vec<Prop> {
    let mut props = vec![
        // The bay door in the rear wall, its frame and warning stripes.
        decor([-13.0, DECK, -63.98], [13.0, 3.0, -63.9], Metal),
        decor([-13.4, 3.0, -63.98], [13.4, 3.4, -63.86], Hazard),
        decor([-0.05, DECK, -63.9], [0.05, 3.0, -63.86], Rail),
        // The travelling crane.
        decor([-24.0, 3.3, -54.6], [24.0, 3.9, -53.8], Metal),
        decor([-3.4, 2.5, -55.2], [-1.4, 3.3, -53.2], Hazard),
        glow(-2.4, -54.2, 0.06, [-0.2, 2.5], Rail),
        decor([-3.0, -0.6, -54.8], [-1.8, -0.2, -53.6], Metal),
        // The way in from the corridor is kept clear and marked.
        decor([-3.1, -3.98, -64.0], [-2.9, -3.96, -44.0], Hazard),
        decor([2.9, -3.98, -64.0], [3.1, -3.96, -44.0], Hazard),
    ];
    let mut side = vec![
        decor([24.0, 3.3, -64.0], [24.5, 3.9, -44.0], Metal),
        decor([8.0, 3.72, -62.0], [8.3, 3.98, -46.0], Lamp),
        decor([18.0, 3.72, -62.0], [18.3, 3.98, -46.0], Lamp),
    ];
    // Heights of the stacks, so that no two neighbours look alike.
    const RISE: [[u32; 4]; 3] = [[3, 2, 4, 1], [2, 4, 1, 3], [4, 1, 3, 2]];
    for (row, z) in [-62.5, -56.5, -50.5].into_iter().enumerate() {
        for (column, x) in [4.5, 10.5, 15.5, 20.5].into_iter().enumerate() {
            let wide = if column == 0 { 5.0 } else { 3.8 };
            side.extend(stack(x, z, wide, 4.0, RISE[row][column]));
        }
    }
    props.extend(twinned(side));
    props
}

/// Everything fixed in place. Door leaves come from the rooms: `door_leaves`.
pub fn furnishings() -> Vec<Prop> {
    let mut props = bridge();
    props.extend(corridors());
    props.extend(quarters());
    props.extend(mess());
    props.extend(twinned(gun_bay()));
    props.extend(sickbay());
    props.extend(airlock());
    props.extend(reactor());
    props.extend(cargo());
    props
}

/// The two leaves of the door in each doorway, sliding apart along its wall.
pub fn door_leaves(rooms: &[Room]) -> Vec<Prop> {
    let mut leaves = Vec::new();
    for room in rooms.iter().filter(|r| r.kind == RoomKind::Doorway) {
        let size = room.max - room.min;
        let door = (room.min + room.max) / 2.0;
        // Across the thin side of the doorway; the leaves run along the long one.
        let (thin, long) = if size.x < size.z { (DVec3::X, DVec3::Z) } else { (DVec3::Z, DVec3::X) };
        let half = long * (size.dot(long) / 2.0);
        let depth = thin * 0.1;
        let height = DVec3::Y * size.y;
        let bottom = door - DVec3::Y * (size.y / 2.0);
        for side in [-1.0, 1.0] {
            let (a, b) = (bottom + half * side, bottom);
            let (low, high) = (a.min(b) - depth, a.max(b) + depth + height);
            leaves.push(Prop {
                min: low,
                max: high,
                material: Door,
                shape: Shape::Block,
                solid: false,
                slide: half * side,
                door,
            });
        }
    }
    leaves
}
