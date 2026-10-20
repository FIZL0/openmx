# S46 — the biplane's ray probe

2026-10-20. Task: `./kit plan S46`.

The plane's ray probe is ported as `game_sim::ray_probe`. Its 36 segments, kept in a tree, are
descended against a mesh's tree of triangles. The port covers the clear and the pair against a mesh
and a placed group. The pair keeps one contact per segment, carries the push and its bounds, and takes
the contacts to the world through the struck mesh.

It was proved by oracle on C7's save state, with the plane's own probe and all 34 candidates its search
gathered. The whole pair was run over 10 000 cases, each record compared bit for bit. The pieces were
run separately, 40 000 cases each: the box test, the segment test and the mixed step, plus 10 000 for
the group gate. The prepare, the probe's extra-object paths, kinds 5 and 6, and smooth normals are not
ported; none occurs for the plane.
