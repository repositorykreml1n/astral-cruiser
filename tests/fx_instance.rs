#[path = "../mixins/astral_cruiser/fx_instance.rs"]
mod astral_fx_instance;

use astral_fx_instance::{
    kind_region, position_at, FxInstance, FxLight, FxPool, FxSpawn, Prim, FIREBALL_CAP,
    FX_CAPACITY, FX_SLOTS, KIND_CAPACITIES, LIGHTS,
};
use glam::DVec3;

#[test]
fn instance_is_64_bytes() {
    assert_eq!(std::mem::size_of::<FxInstance>(), 64);
}

#[test]
fn expiry_alone_does_not_upload_instances() {
    let mut pool = FxPool::new();
    pool.push(FxSpawn {
        prim: Prim::Spark, at: DVec3::ZERO, vec: DVec3::ZERO,
        size: 1.0, grow: 0.0, color: [1.0; 4], born: 0.0, life: 0.1, seed: 1,
    });
    assert!(pool.encode(DVec3::ZERO).is_some());
    pool.expire(1.0);
    assert!(pool.encode(DVec3::ZERO).is_none(), "expiry must not upload dead slots");
}

#[test]
fn spark_falls_and_smoke_rises() {
    let spark = FxInstance {
        pos: [0.0, 10.0, 0.0],
        size: 1.0,
        vec: [0.0, 0.0, 0.0],
        born: 0.0,
        color: [1.0; 4],
        kind: Prim::Spark as u32,
        seed: 0,
        life: 5.0,
        grow: 0.0,
    };
    let smoke = FxInstance {
        pos: [0.0, 10.0, 0.0],
        size: 1.0,
        vec: [0.0, 0.0, 0.0],
        born: 0.0,
        color: [1.0; 4],
        kind: Prim::Smoke as u32,
        seed: 0,
        life: 5.0,
        grow: 0.0,
    };
    let spark_pos_1s = position_at(&spark, 1.0);
    let smoke_pos_1s = position_at(&smoke, 1.0);

    assert!(
        spark_pos_1s.y < spark.pos[1],
        "spark y {} should be < born y {}",
        spark_pos_1s.y,
        spark.pos[1]
    );
    assert!(
        smoke_pos_1s.y > smoke.pos[1],
        "smoke y {} should be > born y {}",
        smoke_pos_1s.y,
        smoke.pos[1]
    );
}

#[test]
fn expire_updates_live_count_without_marking_slots_dirty() {
    let mut pool = FxPool::new();
    let cam = DVec3::new(100.0, 64.0, 100.0);
    pool.push(FxSpawn {
        prim: Prim::Spark,
        at: cam,
        vec: DVec3::ZERO,
        size: 1.0,
        grow: 0.0,
        color: [1.0; 4],
        born: 0.0,
        life: 1.0,
        seed: 42,
    });
    pool.light(FxLight {
        at: cam,
        radius: 10.0,
        color: [1.0, 1.0, 1.0],
        born: 0.0,
        life: 1.0,
    });

    let res = pool.encode(cam);
    assert!(res.is_some());
    assert_eq!(res.unwrap().1.len(), FX_SLOTS);

    // At now = 0.5: item and light are alive (0.5 <= 1.0)
    pool.expire(0.5);
    assert_eq!(pool.len(), 1);
    assert_eq!(pool.lights.len(), 1);
    assert!(
        pool.encode(cam).is_none(),
        "encode should be None when nothing was dropped"
    );

    // At now = 1.5: item and light have expired (1.5 > 1.0)
    pool.expire(1.5);
    assert_eq!(pool.len(), 0);
    assert_eq!(pool.lights.len(), 0);
    let after_expire = pool.encode(cam);
    assert!(
        after_expire.is_none(),
        "expiry must not change GPU records"
    );
    assert!(pool.upload_ranges().is_empty());
    assert!(pool.draw_ranges().iter().all(|range| range.is_empty()));
}

#[test]
fn anchor_moves_without_shifting_world_positions() {
    let mut pool = FxPool::new();
    let world_pos = DVec3::new(10000.0, 70.0, 10000.0);
    pool.push(FxSpawn {
        prim: Prim::Glow,
        at: world_pos,
        vec: DVec3::ZERO,
        size: 2.0,
        grow: 0.0,
        color: [1.0; 4],
        born: 0.0,
        life: 10.0,
        seed: 7,
    });

    let cam1 = world_pos;
    let (anchor1, instances1) = pool.encode(cam1).expect("first encode should return Some");
    let slot = kind_region(Prim::Glow as usize).start;
    assert_eq!(instances1.len(), FX_SLOTS);
    let world1 = anchor1
        + DVec3::new(
            instances1[slot].pos[0] as f64,
            instances1[slot].pos[1] as f64,
            instances1[slot].pos[2] as f64,
        );
    assert!(
        (world1 - world_pos).length() < 0.01,
        "world1 {world1:?} not within 0.01 of world_pos {world_pos:?}"
    );

    let cam2 = cam1 + DVec3::new(5000.0, 0.0, 0.0);
    let (anchor2, instances2) = pool
        .encode(cam2)
        .expect("second encode after 5000 blocks should return Some");
    assert_eq!(instances2.len(), FX_SLOTS);
    assert_ne!(anchor1, anchor2, "anchor should have moved");
    let world2 = anchor2
        + DVec3::new(
            instances2[slot].pos[0] as f64,
            instances2[slot].pos[1] as f64,
            instances2[slot].pos[2] as f64,
        );
    assert!(
        (world2 - world_pos).length() < 0.01,
        "world2 {world2:?} not within 0.01 of world_pos {world_pos:?}"
    );
}

#[test]
fn overflow_drops_oldest_and_caps_fireballs() {
    let mut pool = FxPool::new();
    let origin = DVec3::new(0.0, 64.0, 0.0);

    for i in 0..33000 {
        pool.push(FxSpawn {
            prim: Prim::Spark,
            at: origin,
            vec: DVec3::ZERO,
            size: 1.0,
            grow: 0.0,
            color: [1.0; 4],
            born: 0.0,
            life: 100.0,
            seed: i as u32,
        });
    }

    let cap = KIND_CAPACITIES[Prim::Spark as usize];
    assert_eq!(pool.len(), cap, "overflow is per kind");
    let (_, instances) = pool.encode(origin).expect("encode should return Some");
    let instances = &instances[kind_region(Prim::Spark as usize)];
    assert_eq!(instances.len(), cap);
    let has_seed_0 = instances.iter().any(|inst| inst.seed == 0);
    assert!(!has_seed_0, "seed 0 should have been dropped on overflow");
    assert_eq!(instances.iter().map(|i| i.seed).min(), Some((33000 - cap) as u32));
    assert_eq!(instances.iter().map(|i| i.seed).max(), Some(32999));

    let mut fire_pool = FxPool::new();
    for i in 0..100 {
        fire_pool.push(FxSpawn {
            prim: Prim::Fireball,
            at: origin,
            vec: DVec3::ZERO,
            size: 5.0,
            grow: 1.0,
            color: [1.0, 0.5, 0.1, 1.0],
            born: i as f32,
            life: 10.0,
            seed: i as u32,
        });
    }
    assert_eq!(fire_pool.len(), FIREBALL_CAP);
    let (_, fire_instances) = fire_pool.encode(origin).expect("encode should return Some");
    let fire_instances = &fire_instances[kind_region(Prim::Fireball as usize)];
    assert_eq!(fire_instances.len(), FIREBALL_CAP);
    assert_eq!(fire_instances.len(), 64);
    assert_eq!(fire_instances.iter().map(|i| i.seed).min(), Some(36));
    assert_eq!(fire_instances.iter().map(|i| i.seed).max(), Some(99));
}

#[test]
fn lights_pick_strongest_eight() {
    let mut pool = FxPool::new();
    let cam = DVec3::new(0.0, 64.0, 0.0);

    for i in 1..=12 {
        pool.light(FxLight {
            at: cam + DVec3::new(i as f64, 0.0, 0.0),
            radius: (i * 10) as f32,
            color: [1.0, 0.8, 0.6],
            born: 0.0,
            life: 10.0,
        });
    }

    let lights = pool.lights_at(0.0, cam);
    assert_eq!(lights.len(), LIGHTS);
    assert_eq!(LIGHTS, 8);

    let expected_radii = [120.0, 110.0, 100.0, 90.0, 80.0, 70.0, 60.0, 50.0];
    for (idx, &expected) in expected_radii.iter().enumerate() {
        assert_eq!(
            lights[idx][3], expected,
            "light {idx} radius {} did not match expected {expected}",
            lights[idx][3]
        );
        assert_eq!(lights[idx][7], 1.0);
    }

    let mut pool2 = FxPool::new();
    pool2.light(FxLight {
        at: cam,
        radius: 25.0,
        color: [0.5, 0.5, 1.0],
        born: 0.0,
        life: 5.0,
    });
    let lights2 = pool2.lights_at(0.0, cam);
    assert_eq!(lights2[0][3], 25.0);
    for k in 1..LIGHTS {
        assert_eq!(lights2[k], [0.0; 8], "entry {k} should be all zeros");
    }
}

#[test]
fn encode_is_none_when_nothing_changed() {
    let mut pool = FxPool::new();
    let cam = DVec3::new(50.0, 64.0, -30.0);

    assert!(pool.is_empty());
    assert!(pool.encode(DVec3::ZERO).is_none());

    pool.push(FxSpawn {
        prim: Prim::Beam,
        at: cam,
        vec: cam + DVec3::new(10.0, 0.0, 0.0),
        size: 1.0,
        grow: 0.0,
        color: [1.0, 1.0, 1.0, 1.0],
        born: 0.0,
        life: 2.0,
        seed: 123,
    });

    pool.push(FxSpawn {
        prim: Prim::Shell,
        at: cam,
        vec: DVec3::new(0.0, 1.0, 0.0),
        size: 5.0,
        grow: 2.0,
        color: [1.0, 0.5, 0.2, 1.0],
        born: 0.0,
        life: 1.0,
        seed: 456,
    });

    let res = pool.encode(cam);
    assert!(res.is_some());
    let (_, instances) = res.unwrap();
    assert_eq!(instances.len(), FX_SLOTS);
    // Shell axis must be unchanged
    assert_eq!(instances[kind_region(Prim::Shell as usize).start].vec, [0.0, 1.0, 0.0]);

    assert!(pool.encode(cam).is_none());
    assert!(pool.encode(cam + DVec3::new(50.0, 0.0, 50.0)).is_none());
}

fn spawn(prim: Prim, seed: u32) -> FxSpawn {
    FxSpawn { prim, at: DVec3::ZERO, vec: DVec3::ZERO, size: 1.0,
        grow: 0.0, color: [1.0; 4], born: 0.0, life: 10.0, seed }
}

#[test]
fn one_spark_after_quiet_frame_uploads_exactly_one_slot() {
    let mut pool = FxPool::new();
    pool.push(spawn(Prim::Spark, 1));
    pool.encode(DVec3::ZERO).unwrap();
    assert!(pool.encode(DVec3::ZERO).is_none());
    pool.push(spawn(Prim::Spark, 2));
    pool.encode(DVec3::ZERO).unwrap();
    let start = kind_region(Prim::Spark as usize).start + 1;
    assert_eq!(pool.upload_ranges(), &[start..start + 1]);
    assert_eq!(pool.cache[start].seed, 2);
    assert_eq!(pool.cache[start - 1].seed, 1);
}

#[test]
fn wrapped_writes_coalesce_and_overflow_stays_inside_its_kind() {
    let mut pool = FxPool::new();
    let region = kind_region(Prim::Spark as usize);
    pool.push(spawn(Prim::Glow, 99));
    for seed in 0..region.len() - 2 { pool.push(spawn(Prim::Spark, seed as u32)); }
    pool.encode(DVec3::ZERO).unwrap();
    for seed in 0..5 { pool.push(spawn(Prim::Spark, 100_000 + seed)); }
    pool.encode(DVec3::ZERO).unwrap();
    assert_eq!(pool.upload_ranges(), &[region.end - 2..region.end, region.start..region.start + 3]);
    assert_eq!(pool.len(), region.len() + 1);
    assert_eq!(pool.cache[kind_region(Prim::Glow as usize).start].seed, 99);
    // More than a complete lap needs a single upload of this region only.
    for seed in 0..region.len() + 7 { pool.push(spawn(Prim::Spark, seed as u32)); }
    pool.encode(DVec3::ZERO).unwrap();
    assert_eq!(pool.upload_ranges(), &[region]);
    assert_eq!(pool.cache[kind_region(Prim::Glow as usize).start].seed, 99);
}

#[test]
fn regions_fit_and_recreated_gpu_gets_retained_records() {
    assert_eq!(KIND_CAPACITIES.iter().sum::<usize>(), FX_SLOTS);
    assert!(FX_SLOTS <= FX_CAPACITY);
    let mut pool = FxPool::new();
    pool.push(spawn(Prim::Beam, 42));
    pool.encode(DVec3::ZERO).unwrap();
    pool.reupload();
    pool.encode(DVec3::ZERO).unwrap();
    assert_eq!(pool.upload_ranges(), &[0..FX_SLOTS]);
    assert_eq!(pool.cache[0].seed, 42);
    assert!(pool.encode(DVec3::ZERO).is_none());
}

#[test]
fn anchor_move_preserves_beam_endpoint_and_shell_axis_but_never_revives_dead_slots() {
    let mut pool = FxPool::new();
    let mut beam = spawn(Prim::Beam, 1);
    beam.at = DVec3::new(10000.125, 70.0, 10000.0);
    beam.vec = beam.at + DVec3::new(300.25, 1.0, -2.0);
    pool.push(beam);
    pool.push(FxSpawn { vec: DVec3::Y, ..spawn(Prim::Shell, 2) });
    pool.push(FxSpawn { life: 0.1, ..spawn(Prim::Glow, 3) });
    pool.encode(beam.at).unwrap();
    pool.expire(0.2); // Dead glow remains in CPU storage until the next count scan.
    pool.encode(beam.at + DVec3::X * 5000.0).unwrap();
    assert_eq!(pool.upload_ranges(), &[0..FX_SLOTS]);
    let encoded = pool.cache[kind_region(Prim::Beam as usize).start];
    assert!((pool.anchor + glam::Vec3::from(encoded.pos).as_dvec3() - beam.at).length() < 0.01);
    assert!((pool.anchor + glam::Vec3::from(encoded.vec).as_dvec3() - beam.vec).length() < 0.01);
    assert_eq!(pool.cache[kind_region(Prim::Shell as usize).start].vec, [0.0, 1.0, 0.0]);
    assert_eq!(pool.cache[kind_region(Prim::Glow as usize).start].life, 0.0);
    assert!(pool.encode(beam.at + DVec3::X * 5000.0).is_none());
}

#[test]
fn latest_death_keeps_delayed_records_and_skips_a_dead_kind_without_uploading() {
    let mut pool = FxPool::new();
    pool.push(FxSpawn { born: 2.0, life: 2.0, ..spawn(Prim::Glow, 1) });
    pool.push(FxSpawn { life: 0.1, ..spawn(Prim::Glow, 2) });
    pool.encode(DVec3::ZERO).unwrap();
    pool.expire(0.6);
    assert_eq!(pool.len(), 1);
    let region = kind_region(Prim::Glow as usize);
    assert_eq!(pool.draw_ranges()[Prim::Glow as usize], region.start as u32..region.end as u32);
    assert!(pool.encode(DVec3::ZERO).is_none());
    pool.expire(4.01);
    assert_eq!(pool.len(), 0);
    assert!(pool.draw_ranges()[Prim::Glow as usize].is_empty());
    assert!(pool.encode(DVec3::ZERO).is_none());
}
