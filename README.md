# Astral Cruiser

A native mod for Horizon: a huge pilotable military starship with a walkable interior, lasers and strikes, coordinate dash and jump, and a portal that shows the real destination dimension live before you cross it, on foot or aboard.

Start with `/astral spawn`, then `/astral board`. N opens the control panel; keys are in Options > Mods > Settings.

## What's in it

- **The ship.** A 160-block cruiser: armour belts, stepped superstructure, light stripes, red, green and white beacons, engine nacelles with glowing thrust.
- **Two decks inside**, walkable while the ship moves: bridge, reactor hall, weapons room, quarters, mess, medbay and hangar below; captain's cabin, arsenal, gym and a gallery over the reactor above; an observation dome on top, all joined by stairs.
- **Weapons.** Turrets, nose guns and beams, and a weapon wheel of strikes: Barrage, Rod, Lance, Antimatter, Solar Pillar, Tectonic, Meteors, Cluster, Cryo, Black Hole, Chain, Plasma Wall, Missiles, Drill, Grid, Erasure and Supernova. Each has its own fire, smoke, sparks, shockwaves and light on the hull. Strikes reach at most 384 blocks from the ship.
- **Travel.** Coordinate dash, jump and orbit.
- **Portals.** A portal shows the destination dimension live, and you cross it aboard or on foot.

## What it does as a native mod

The launcher compiles it into the game, without a sandbox. It changes the game's rendering, ticking and chunk loading for the ship, its effects and the portals.

It adds no network access and reads no accounts or files outside the game's own data. It keeps the ship in the world's own folder (`astral_cruiser/ship.json`). It holds no Minecraft code or assets.

Strikes break blocks in the world where they land, so try them in a test world first. The nose guns and beams break blocks only with blasting turned on in the control panel (off by default).

## Building

Built for Horizon commit `68ac6f647e7022b2c696e81863d7cface28c9416`, Minecraft 26.3: the mod hooks the renderer, so it is pinned to that commit. The bundle is in `mixins/astral_cruiser`, and the unit tests run with `cargo test`.

Licence: MIT.
