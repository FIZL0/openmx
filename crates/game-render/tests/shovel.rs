//! S267: free10's shovel off the user's disc, the one model besides the cars whose meshes carry their
//! own node lists (flag 0x80, S245). Each vertex is on the node its mesh's list names: it lies in that
//! node's bounds, as the cars' do. Skips without a disc.

use game_render::Disc;
use game_render::animated_toy::{Animated, place};
use game_render::psg::Model;

#[test]
fn free10s_shovel_vertices_lie_in_their_nodes() {
    let mut disc = match Disc::open_configured() {
        Ok(d) => d,
        Err(why) => {
            eprintln!("skipped: {why}");
            return;
        }
    };
    let names: Vec<String> = disc
        .entries()
        .iter()
        .map(|e| e.name.clone())
        .filter(|n| {
            let n = n.to_ascii_lowercase();
            n.ends_with("\\free10.res") || n.ends_with("\\free10_2.res")
        })
        .collect();
    let mut archives: Vec<_> = names.iter().map(|n| disc.res(n).unwrap_or_else(|e| panic!("{n}: {e:?}"))).collect();
    let bytes = archives.iter().find_map(|a| a.read_suffix("\\shovel.psg").ok()).expect("free10 has shovel.psg");
    let model = Model::parse(&bytes).expect("shovel.psg parses");
    let mut outside = Vec::new();
    let mut count = 0;
    for mesh in &model.meshes {
        for v in mesh.strips.iter().flat_map(|s| s.verts.iter()) {
            count += 1;
            let n = &model.nodes[v.node];
            let off = v.pos.sub(n.center);
            let out = [(off.x, n.half_extents.x), (off.y, n.half_extents.y), (off.z, n.half_extents.z)].iter().any(|(o, h)| o.abs() > h + 0.05);
            if out {
                outside.push(format!("{} {:?}", n.name, v.pos));
            }
        }
    }
    assert!(count > 0, "the shovel has no vertices");
    assert!(outside.is_empty(), "{} of {count} shovel vertices outside their node's bounds: {:?}", outside.len(), &outside[..outside.len().min(5)]);

    // Where the shell stands it, for a shot of it (`--shot-pose`).
    let animated = Animated::load(&mut archives, "free10");
    let k = animated.names.iter().position(|n| n.to_ascii_lowercase().contains("shovel")).expect("free10's .toy places the shovel");
    let at = place(&animated.sim, k);
    println!("shovel: {} nodes, {count} vertices, at {:?}", model.nodes.len(), at.0[3]);
}
