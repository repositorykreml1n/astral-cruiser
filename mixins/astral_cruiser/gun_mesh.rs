// Battery cuboids share the hull material pipeline; glow uses the light pass.
use super::astral_guns::{Battery, Guns};
use horizon_client::gfx::astral_hull::{HullVertex, Bolt, prop_vertices};
use horizon_client::gfx::astral_layout::{Material, Prop, Shape};
use glam::{DQuat, DVec3};

pub fn mesh(guns: &Guns) -> Vec<HullVertex> {
    let mut out = Vec::new();
    for gun in &guns.0 {
        let pose = gun.pose();
        let w = gun.battery.width();
        let sides: &[f64] = if gun.battery == Battery::MissilePods { &[1.0, -1.0] } else { &[1.0] };
        for &side in sides {
            let shift = DVec3::new(pose.mount.x * (side - 1.0), 0.0, 0.0);
            let mut block = |center: DVec3, half: DVec3, rotation: DQuat, material| {
                let prop = Prop { min: -half, max: half, material, shape: Shape::Block, solid: false, slide: DVec3::ZERO, door: DVec3::ZERO };
                let mut vertices = prop_vertices(&prop);
                for v in &mut vertices {
                    v.position = (center + shift + rotation * DVec3::from_array(v.position.map(f64::from))).as_vec3().to_array();
                    v.normal = (rotation * DVec3::from_array(v.normal.map(f64::from))).as_vec3().to_array();
                }
                out.extend(vertices);
            };
            for sign in [-1.0, 1.0] {
                block(pose.mount + DVec3::new(sign * w * (0.5 + pose.hatch), -0.2, 0.0), DVec3::new(w * 0.5, 0.3, w * 1.4), DQuat::IDENTITY, Material::Armor);
            }
            if pose.extension <= 0.0 { continue; }
            block(pose.pivot, DVec3::new(w, pose.extension, w), DQuat::IDENTITY, Material::Turret);
            let length = gun.battery.length() * pose.extension;
            let offsets: &[f64] = match gun.battery { Battery::Railgun => &[-0.7, 0.7], Battery::MissilePods => &[-1.8, 0.0, 1.8], _ => &[0.0] };
            for &x in offsets {
                let half_width = if gun.battery == Battery::Silo { 2.5 } else { 0.55 };
                // Four walls leave a visible open bore instead of a solid tube end.
                for (offset, half) in [
                    (DVec3::new(-half_width, 0.0, 0.0), DVec3::new(0.22, half_width, length / 2.0)),
                    (DVec3::new(half_width, 0.0, 0.0), DVec3::new(0.22, half_width, length / 2.0)),
                    (DVec3::new(0.0, -half_width, 0.0), DVec3::new(half_width, 0.22, length / 2.0)),
                    (DVec3::new(0.0, half_width, 0.0), DVec3::new(half_width, 0.22, length / 2.0)),
                ] {
                    let local = DVec3::new(x, 0.0, length / 2.0 - pose.recoil) + offset;
                    block(pose.pivot + pose.rotation * local, half, pose.rotation, Material::Turret);
                }
            }
        }
    }
    out
}
#[allow(dead_code)]
pub fn light(guns: &Guns, position: DVec3, rotation: DQuat) -> Vec<Bolt> {
    let mut out = Vec::new();
    for gun in &guns.0 {
        let p = gun.pose();
        if p.glow <= 0.0 || p.extension < 1.0 { continue; }
        let sides: &[f64] = if gun.battery == Battery::MissilePods { &[1.0, -1.0] } else { &[1.0] };
        for &side in sides {
            let at = position + rotation * (p.muzzle + DVec3::new(p.mount.x * (side - 1.0), 0.0, 0.0));
            out.push(Bolt { from: at, to: at, width: 0.0, flash: (1.0 + p.glow * 3.0) as f32, color: [0.5 + p.glow as f32 * 0.5, 0.65, 1.0, p.glow as f32] });
            if p.recoil > 0.0 {
                for i in 1..=5 {
                    let smoke = at + rotation * p.direction * (i as f64 * (2.0 - p.recoil));
                    out.push(Bolt { from: smoke, to: smoke, width: 0.0, flash: i as f32 * 0.6, color: [0.4, 0.35, 0.3, p.recoil as f32 * 0.2] });
                }
            }
        }
    }
    out
}
