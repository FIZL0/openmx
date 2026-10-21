# S75 — put back clearance

2026-10-21. Task: `./kit plan S75`.

S66's put-back record test answered `World::solid` with nothing, so the clearance's refusals were only
counted. The ride tests' world now answers `solid` with the original's own mesh query (`put_back_solid`,
run in the interpreter on the save state's RAM, with each object's sweep hooked so that every object's
answer is read). The query turned out to refuse for one more reason than a probe hit: a placed group
with a part within 20 ft. The port's clearance now asks a new `World::crowded` for that. On the five
captures nothing solid is ever near the path: every held tick is a slope refusal or a keep in place,
and none moved. So the refusals are proved on 10,000 generated probes around the objects the query
sweeps, on free06 and free09: the port keeps or refuses each one as the original does, and the kept ones
match bit for bit. In game, `solid` and `crowded` are not the original's query yet; that is S80. Details
are in 1-put-back-clearance-FINAL.md.
