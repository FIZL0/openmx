//! MX vs. ATV's cars as the shell draws them (MD157): the car's chassis model off its car pack (the
//! `psg` its `cars` row names, with the pack's own materials and textures) at the car's scene node
//! ([`crate::vehicle::MxvatvCarRide::matrix`]), posed by its four tires off the step — their roll,
//! steer and suspension's compression — on `game_render::car`'s rigs (the truck's for the monster
//! truck, the buggy's arms for the buggy, the baja and the sandrail, the golf cart's own hubs on
//! top). The cars rode undrawn from MD39c to here. Offline only, as the cars are.
//!
//! **The driver** (MD175) sits as MX Unleashed's car driver does (S245): the rider's body posed by
//! the plane pilot's clips, its root at the middle of the model's four seat nodes and
//! [`game_render::car::SEAT_FROM_LIMBS`], its feet and hands held to those nodes. Four cars spell
//! them `Dummy_Foot_L_pos` as MXU's cars do; the golf cart drops the `_pos` (`Dummy_Foot_L`), and
//! SLUS-21104 holds both spellings, so [`seat_nodes`] takes either.

use game_render::car::{Car, SEAT, SEAT_FROM_LIMBS, Tire, node_at};
use game_render::mat::{M4, V3};
use game_render::psg::Model;
use game_render::world::Assets;

use crate::machine_packs::Pack;
use crate::vehicle::{MxvatvCarRide, mxvatv_machine};

/// The key a car pack's chassis model `psg` is kept under in [`Assets`]: its own, so neither MX
/// Unleashed's car of the same name (`car:buggy`) nor another pack's is taken for it.
pub fn key(pack: &str, psg: &str) -> String {
    format!("mxvatv_car:{pack}:{}", stem(psg).to_ascii_lowercase())
}

/// A model path's name without its folders and `.psg`.
fn stem(psg: &str) -> &str {
    let leaf = psg.rsplit(['\\', '/']).next().unwrap_or(psg);
    leaf.strip_suffix(".psg").or_else(|| leaf.strip_suffix(".PSG")).unwrap_or(leaf)
}

/// Load the chassis of `car` (its pack among `packs`) into `assets`: the model its pack's row names,
/// its materials read off that pack alone. The model as [`Car`] keeps it, and its path in the pack.
pub fn load(assets: &mut Assets, packs: &[Pack], car: &MxvatvCarRide) -> Result<(Car, String), String> {
    let pack = packs.iter().find(|p| p.id == car.pack).ok_or_else(|| format!("no pack {}", car.pack))?;
    let row = pack.cars.iter().find(|c| mxvatv_machine(&c.car) == Some(car.machine)).ok_or_else(|| format!("{} lists no {}", pack.id, car.label()))?;
    let key = key(&pack.id, &row.psg);
    // Repainted with nothing: the material answers MX Unleashed's own cars left are not reused.
    if !assets.load_repainted(std::slice::from_ref(&pack.files), stem(&row.psg), &key, &[]) {
        return Err(format!("{}: {} did not load", pack.id, row.psg));
    }
    Ok((Car { model: key }, row.psg.clone()))
}

/// The four tires as the pose reads them off the car out: each tire's roll angle, its steer and its
/// suspension's compression. The step has no `MaxTireScale`, so the tires do not stretch. At rest for
/// a car not out.
pub fn tires(car: &MxvatvCarRide) -> [Tire; 4] {
    let Some(r) = car.ridden.as_ref() else { return [Tire::REST; 4] };
    std::array::from_fn(|k| {
        let s = &r.state.tires[k];
        Tire { spin: s.angle.0, steer: s.steer.0, compression: s.compression.0, scale: 1.0 }
    })
}

/// Where the car out is drawn: its scene node's matrix, the model's placement (rows x, y, z, origin).
/// `None` before its first step has placed the node (all zeros), so the tick it is taken on does not
/// slide it in from the world's origin (MD175).
pub fn placed(car: &MxvatvCarRide) -> Option<M4> {
    let m = car.matrix()?;
    if m.0[3].0[3] == 0.0 {
        return None;
    }
    Some(M4(std::array::from_fn(|r| std::array::from_fn(|c| m.0[r].0[c]))))
}

/// The four seat nodes the driver's feet and hands are held to, in [`SEAT`]'s order, where each sits
/// in the car's space: each as the model spells it, with `_pos` or (the golf cart) without. `None`
/// for a model lacking one.
pub fn seat_nodes(model: &Model) -> Option<[V3; 4]> {
    let at = |name: &str| node_at(model, name).or_else(|| node_at(model, name.strip_suffix("_pos")?));
    let [a, b, c, d] = SEAT.map(at);
    Some([a?, b?, c?, d?])
}

/// Where the driver's root sits in the car's space: the middle of the four [`seat_nodes`] and
/// [`SEAT_FROM_LIMBS`], as MX Unleashed's car's ([`game_render::car::seat`]).
pub fn seat(model: &Model) -> Option<V3> {
    let n = seat_nodes(model)?;
    Some(n[0].add(n[1]).add(n[2]).add(n[3]).scale(0.25).add(SEAT_FROM_LIMBS))
}

/// `--trace-picture`'s line for a car out drawn this frame: the renderer, the model, where its origin
/// stands, how many triangles it draws and its tires.
pub fn trace(p: usize, renderer: &str, drawn: &Car, assets: &Assets, at: M4, tires: &[Tire; 4]) {
    let o = at.0[3];
    let tris = assets.models.get(&drawn.model).map_or(0, |l| l.tris.len());
    let t: Vec<String> = tires.iter().map(|t| format!("{:.2}/{:.2}/{:.2}", t.spin, t.steer, t.compression)).collect();
    println!("picture mxvatv car: seat {p} {renderer} {} at {:.2} {:.2} {:.2} triangles {tris} tires {}", drawn.model, o[0], o[1], o[2], t.join(" "));
}
