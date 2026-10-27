rows 12917, frames 127882..140798
gaps []
P1 guard misses 0 []
P2 guard misses 0 []

## Wrecks

| frame | row | rider | reason | clause | value | speed before | new entries (kind, other, ray end y) |
|---|---|---|---|---|---|---|---|
| 128995 | 1113 | P1 | 8 NoseDive | airborne roll | pitch -54.71 deg (limit -80), roll -85.03 deg (limit +-85) (row before: pitch -53.65 deg (limit -80), roll -84.85 deg (limit +-85)) | 55.8 | [] |
| 129945 | 2063 | P1 | 8 NoseDive | airborne pitch down | pitch -80.53 deg (limit -80), roll 0.11 deg (limit +-85) (row before: pitch -79.68 deg (limit -80), roll 0.10 deg (limit +-85)) | 79.2 | [] |
| 130966 | 3084 | P1 | 11 Tail | airborne pitch up / roll | pitch 85.33 deg (limit 85), roll 2.45 deg (limit +-85) (row before: flip_torqued 0, pitch 84.48 deg) | 69.5 | [] |
| 132997 | 5115 | P1 | 3 Right | landing crossup | hang time 1.385 s (crossup itself not captured) (row before: hang time 1.385 s (crossup itself not captured)) | 47.3 | [] |
| 133313 | 5431 | P1 | 12 NoHander | landed inside a trick (trick ending) | trick 12 rider_kod still running (row before: trick 12 rider_kod still running) | 46.3 | [] |
| 133953 | 6071 | P1 | 12 NoHander | landed inside a trick (trick ending) | trick 5 rider_pendulum still running (row before: trick 5 rider_pendulum still running) | 100.9 | [] |
| 134381 | 6499 | P1 | 12 NoHander | landed inside a trick (trick ending) | trick 3 rider_catwalk still running (row before: trick 3 rider_catwalk still running) | 64.9 | [] |
| 135196 | 7314 | P1 | 12 NoHander | landed inside a trick (trick ending) | trick 6 rider_nacnac still running (row before: trick 6 rider_nacnac still running) | 38.9 | [] |
| 135855 | 7973 | P1 | 12 NoHander | landed inside a trick (trick ending) | trick 0 rider_Taunt still running (row before: trick 0 rider_Taunt still running) | 56.0 | [] |
| 136559 | 8677 | P1 | 12 NoHander | landed inside a trick (trick ending) | trick 3 rider_catwalk still running (row before: trick 3 rider_catwalk still running) | 50.2 | [] |
| 139659 | 11777 | P1 | 2 Left | landing crossup | hang time 0.884 s (crossup itself not captured) (row before: hang time 0.884 s (crossup itself not captured)) | 62.4 | [] |
| 130418 | 2536 | P2 | 6 Scenery | a body ray on scenery | g (2.06, 0.33, -1.28) (row before: g (0.0, 0.0, 0.0)) | 50.0 | [] |
| 130897 | 3015 | P2 | 6 Scenery | a body ray on scenery | g (0.0, 0.0, 0.0) (row before: g (0.0, 0.0, 0.0)) | 74.9 | [] |
| 131595 | 3713 | P2 | 1 Front | ground pitch down | pitch -70.81 deg (limit -70) (row before: pitch -66.34 deg (limit -70)) | 32.6 | [] |
| 133800 | 5918 | P2 | 8 NoseDive | airborne roll | pitch 76.39 deg (limit -80), roll -85.14 deg (limit +-85) (row before: pitch 76.80 deg (limit -80), roll -81.13 deg (limit +-85)) | 38.7 | [] |
| 134315 | 6433 | P2 | 12 NoHander | landed inside a trick (trick ending) | trick 7 rider_flatliner still running (row before: trick 7 rider_flatliner still running) | 65.4 | [] |
| 140363 | 12481 | P2 | 2 Left | landing crossup | hang time 2.019 s (crossup itself not captured) (row before: hang time 2.019 s (crossup itself not captured)) | 72.7 | [] |

## Near misses (ridden out: the biggest loads and attitudes each clause judged that did not wreck)

Riding rows only: no wreck reason within 10 ticks either side and body mode 1 for 10 ticks before (a
put-back teleports the bike). The g clauses read the tick's peak g; ground pitch reads grounded rows;
the airborne pitch and roll limits are judged on a touch, so they read the landing tick (airborne the
row before, not now). A row is a near miss when it reaches half its limit.

| rider | clause | limit | frame | row | value | share of limit |
|---|---|---|---|---|---|---|
| P1 | fore-aft g (front) | 18.33 | (best 131484) | 3602 | 3.03 | 17% |
| P1 | fore-aft g (back) | 22.00 | (best 129486) | 1604 | 0.00 | 0% |
| P1 | side g | 19.07 | (best 131484) | 3602 | 3.84 | 20% |
| P1 | vertical g (flat) | 66.00 | 129486 | 1604 | 55.87 | 85% |
| P1 | vertical g (flat) | 66.00 | 131872 | 3990 | 53.27 | 81% |
| P1 | vertical g (flat) | 66.00 | 132463 | 4581 | 34.33 | 52% |
| P1 | ground pitch down (deg) | 70.00 | 130420 | 2538 | 67.43 | 96% |
| P1 | ground pitch down (deg) | 70.00 | 129485 | 1603 | 60.25 | 86% |
| P1 | ground pitch down (deg) | 70.00 | 129124 | 1242 | 40.54 | 58% |
| P1 | ground pitch up (deg) | 70.00 | 131628 | 3746 | 48.14 | 69% |
| P1 | ground pitch up (deg) | 70.00 | 128520 | 638 | 43.28 | 62% |
| P1 | ground pitch up (deg) | 70.00 | 128892 | 1010 | 43.03 | 61% |
| P1 | touch pitch down (deg) | 80.00 | 130420 | 2538 | 67.43 | 84% |
| P1 | touch pitch down (deg) | 80.00 | 129485 | 1603 | 60.25 | 75% |
| P1 | touch pitch down (deg) | 80.00 | 129123 | 1241 | 40.39 | 50% |
| P1 | touch pitch up (deg) | 85.00 | (best 129081) | 1199 | 35.02 | 41% |
| P1 | touch |roll| (deg) | 85.00 | 131484 | 3602 | 85.95 | 101% |
| P1 | touch |roll| (deg) | 85.00 | 132608 | 4726 | 69.42 | 82% |
| P1 | touch |roll| (deg) | 85.00 | 135076 | 7194 | 50.02 | 59% |
| P2 | fore-aft g (front) | 18.33 | (best 133538) | 5656 | 0.00 | 0% |
| P2 | fore-aft g (back) | 22.00 | (best 128851) | 969 | 0.00 | 0% |
| P2 | side g | 19.07 | (best 128851) | 969 | 0.00 | 0% |
| P2 | vertical g (flat) | 66.00 | 133538 | 5656 | 64.09 | 97% |
| P2 | vertical g (flat) | 66.00 | 128851 | 969 | 48.35 | 73% |
| P2 | vertical g (flat) | 66.00 | 128467 | 585 | 44.55 | 68% |
| P2 | ground pitch down (deg) | 70.00 | 128849 | 967 | 50.33 | 72% |
| P2 | ground pitch down (deg) | 70.00 | 133253 | 5371 | 45.49 | 65% |
| P2 | ground pitch down (deg) | 70.00 | 131576 | 3694 | 41.34 | 59% |
| P2 | ground pitch up (deg) | 70.00 | 136062 | 8180 | 56.90 | 81% |
| P2 | ground pitch up (deg) | 70.00 | 133744 | 5862 | 52.78 | 75% |
| P2 | ground pitch up (deg) | 70.00 | 136142 | 8260 | 50.57 | 72% |
| P2 | touch pitch down (deg) | 80.00 | 128849 | 967 | 50.33 | 63% |
| P2 | touch pitch down (deg) | 80.00 | 131576 | 3694 | 41.34 | 52% |
| P2 | touch pitch up (deg) | 85.00 | 136062 | 8180 | 56.90 | 67% |
| P2 | touch |roll| (deg) | 85.00 | 135556 | 7674 | 49.22 | 58% |
| P2 | touch |roll| (deg) | 85.00 | 137982 | 10100 | 43.00 | 51% |

## World contacts (onsets of record entries on scenery or a record that is no rider, plane or car)

| frame | row | rider | names | kinds | ray end y | normal | speed | wreck after |
|---|---|---|---|---|---|---|---|---|
| 131483 | 3601 | P1 | scenery | [0] | 1.86 | (0.05, 1.0, 0.0) | 73.6 |  |
| 130413 | 2531 | P2 | scenery | [0] | 1.86 | (-0.2, -0.0, -0.98) | 57.0 |  |
| 130417 | 2535 | P2 | scenery | [0] | 4.58 | (-0.2, -0.0, -0.98) | 50.1 | 130418 |
| 130896 | 3014 | P2 | scenery | [0] | 4.58 | (-0.56, 0.0, 0.83) | 74.9 | 130897 |

## Rider-vs-rider contacts

| frame | row | rider | other | kind | kinds | ray end y | speed | other speed | height diff | wreck after (self, other) |
|---|---|---|---|---|---|---|---|---|---|---|

## Contacts with other actors (biplanes, path cars by their collidable)

| frame | row | rider | names | kinds | ray end y | speed | wreck after |
|---|---|---|---|---|---|---|---|

## Truck rides (a wheel on a path car)

| from | to | rows | rider | car | car speed | wheels | stays on |
|---|---|---|---|---|---|---|---|

## Ragdoll (body mode 2) and the put-back

| rider | mode 2 from | to | wreck reason | reason clears | rows |
|---|---|---|---|---|---|
| P1 | 128996 | 129060 | 8 | 129061 | 65 |
| P1 | 129946 | 130033 | 8 | 130033 | 88 |
| P1 | 130967 | 131127 | 11 | 131127 | 161 |
| P1 | 132998 | 133152 | 3 | 133152 | 155 |
| P1 | 133314 | 133472 | 12 | 133472 | 159 |
| P1 | 133954 | 134108 | 12 | 134108 | 155 |
| P1 | 134382 | 134536 | 12 | 134536 | 155 |
| P1 | 135197 | 135351 | 12 | 135351 | 155 |
| P1 | 135856 | 136010 | 12 | 136010 | 155 |
| P1 | 136560 | 136714 | 12 | 136714 | 155 |
| P1 | 139660 | 139814 | 2 | 139814 | 155 |
| P2 | 130419 | 130620 | 6 | 130620 | 202 |
| P2 | 130898 | 131052 | 6 | 131052 | 155 |
| P2 | 131597 | 131754 | 1 | 131754 | 158 |
| P2 | 133802 | 134010 | 8 | 134010 | 209 |
| P2 | 134316 | 134471 | 12 | 134471 | 156 |
| P2 | 140365 | 140519 | 2 | 140519 | 155 |

## Body modes (all stretches that are not 1)

P1: 3 128540..128628, 0 129061..129063, 0 130034..130035, 0 130435..130436, 3 130828..130955, 0 131128..131129, 0 131972..131973, 0 132729..132730, 0 133153..133154, 3 133308..133313, 0 133473..133474, 3 133932..133953, 0 134109..134110, 3 134353..134381, 0 134537..134538, 3 135191..135196, 0 135352..135353, 3 135844..135855, 0 136011..136012, 0 136134..136135, 3 136513..136559, 0 136715..136716, 0 139815..139816, 0 140142..140143
P2: 3 128316..128416, 3 128664..128818, 3 128943..129047, 3 130019..130273, 0 130621..130622, 0 131053..131054, 0 131755..131756, 0 134011..134012, 3 134292..134315, 0 134472..134473, 0 138118..138119, 3 138440..138478, 0 140520..140521

## Tricks (trick_running) and backflips (flip_on)

P1 trick 128539..128626 id 4 machine state 4 rider_nohandedlander 
P1 trick 130827..130953 id 1 machine state 1 rider_lazyboy 
P1 trick 133307..133471 id 12 machine state 12 rider_kod wrecked 12
P1 trick 133931..134107 id 5 machine state 5 rider_pendulum wrecked 12
P1 trick 134352..134535 id 3 machine state 3 rider_catwalk wrecked 12
P1 trick 135190..135350 id 6 machine state 6 rider_nacnac wrecked 12
P1 trick 135843..136009 id 0 machine state 0 rider_Taunt wrecked 12
P1 trick 136512..136713 id 3 machine state 3 rider_catwalk wrecked 12
P2 trick 128315..128413 id 7 machine state 7 rider_flatliner 
P2 trick 128679..128797 id 7 machine state 7 rider_flatliner 
P2 trick 128961..129045 id 2 machine state 2 rider_cancan 
P2 trick 130038..130270 id 7 machine state 7 rider_flatliner 
P2 trick 134291..134470 id 7 machine state 7 rider_flatliner wrecked 12
P2 backflip fired 128653..128798
P2 backflip fired 128931..128948
P2 backflip fired 130016..130269
P2 backflip fired 138422..138444

## Machine states (clip rows) other than riding/air, per rider

P1:
  rider_Taunt (0): 1 runs, 13 rows; 135843..135855
  rider_lazyboy (1): 1 runs, 128 rows; 130827..130954
  rider_catwalk (3): 2 runs, 78 rows; 134352..134381 136512..136559
  rider_nohandedlander (4): 1 runs, 89 rows; 128539..128627
  rider_pendulum (5): 1 runs, 23 rows; 133931..133953
  rider_nacnac (6): 1 runs, 7 rows; 135190..135196
  rider_kod (12): 1 runs, 7 rows; 133307..133313
  turn_riding_Fwd (30): 59 runs, 8747 rows; 127882..128301 128305..128538 128688..128702 128756..128904 129061..129063 129081..129097 129104..129106 129109..129121 129123..129303 129485..129488 129499..129787 130035..130282
  reverse_l (33): 1 runs, 19 rows; 140378..140396
  reverse_r (34): 1 runs, 24 rows; 140354..140377
  air_Fwd (35): 59 runs, 2224 rows; 128302..128304 128628..128687 128703..128755 128905..128995 129064..129080 129098..129103 129107..129108 129122..129122 129304..129484 129489..129498 129788..129945 130034..130034
  wreck_air_flail (40): 11 runs, 851 rows; 128996..129060 129946..130033 130967..131084 132998..133043 133314..133419 133954..134045 134382..134463 135197..135247 135856..135952 136560..136618 139660..139706
  hold_elbow_left (42): 3 runs, 194 rows; 134046..134108 134464..134536 135953..136010
  hold_elbow_right (43): 1 runs, 96 rows; 136619..136714
  hold_knee_left (44): 2 runs, 152 rows; 131085..131127 133044..133152
  hold_knee_right (45): 3 runs, 265 rows; 133420..133472 135248..135351 139707..139814
P2:
  rider_cancan (2): 1 runs, 86 rows; 128961..129046
  rider_flatliner (7): 4 runs, 481 rows; 128315..128415 128679..128798 130038..130272 134291..134315
  turn_riding_Fwd (30): 58 runs, 9532 rows; 127882..128296 128466..128610 128634..128661 128849..128940 128950..128953 129103..129254 129262..129263 129267..129793 129804..130016 130275..130286 130349..130395 130621..130707
  reverse_l (33): 4 runs, 107 rows; 132148..132152 138873..138938 138977..138991 139023..139043
  reverse_r (34): 6 runs, 157 rows; 132128..132147 132646..132688 138871..138872 138939..138976 138992..139022 139044..139066
  air_Fwd (35): 56 runs, 1471 rows; 128297..128314 128416..128465 128611..128633 128662..128663 128800..128848 128941..128942 128954..128960 129047..129102 129255..129261 129264..129266 129794..129803 130017..130018
  backflip (38): 5 runs, 48 rows; 128664..128678 128799..128799 128943..128949 130019..130037 138440..138445
  wreck_air_flail (40): 6 runs, 582 rows; 130419..130537 130898..131017 131597..131671 133802..133927 134316..134388 140365..140433
  hold_head (41): 1 runs, 86 rows; 140434..140519
  hold_elbow_left (42): 3 runs, 249 rows; 130538..130620 131672..131754 134389..134471
  hold_elbow_right (43): 2 runs, 118 rows; 131018..131052 133928..134010

## Animations: foot down, reverse, put-backs, lean extremes

P1 reverse                            140354..140396 (43 rows) reverse_l, reverse_r
P1 wreck flail                        128996..129060 (65 rows) wreck_air_flail
P1 wreck flail                        129946..130033 (88 rows) wreck_air_flail
P1 wreck flail                        130967..131084 (118 rows) wreck_air_flail
P1 wreck flail                        132998..133043 (46 rows) wreck_air_flail
P1 wreck flail                        133314..133419 (106 rows) wreck_air_flail
P1 wreck flail                        133954..134045 (92 rows) wreck_air_flail
P1 wreck flail                        134382..134463 (82 rows) wreck_air_flail
P1 wreck flail                        135197..135247 (51 rows) wreck_air_flail
P1 wreck flail                        135856..135952 (97 rows) wreck_air_flail
P1 wreck flail                        136560..136618 (59 rows) wreck_air_flail
P1 wreck flail                        139660..139706 (47 rows) wreck_air_flail
P1 hold (after a wreck)               131085..131127 (43 rows) hold_knee_left
P1 hold (after a wreck)               133044..133152 (109 rows) hold_knee_left
P1 hold (after a wreck)               133420..133472 (53 rows) hold_knee_right
P1 hold (after a wreck)               134046..134108 (63 rows) hold_elbow_left
P1 hold (after a wreck)               134464..134536 (73 rows) hold_elbow_left
P1 hold (after a wreck)               135248..135351 (104 rows) hold_knee_right
P1 hold (after a wreck)               135953..136010 (58 rows) hold_elbow_left
P1 hold (after a wreck)               136619..136714 (96 rows) hold_elbow_right
P1 hold (after a wreck)               139707..139814 (108 rows) hold_knee_right
P1 put-back (body mode 0)             129061..129063 (3 rows) after mode 2
P1 put-back (body mode 0)             130034..130035 (2 rows) after mode 2
P1 put-back (body mode 0)             130435..130436 (2 rows) after mode 1
P1 put-back (body mode 0)             131128..131129 (2 rows) after mode 2
P1 put-back (body mode 0)             131972..131973 (2 rows) after mode 1
P1 put-back (body mode 0)             132729..132730 (2 rows) after mode 1
P1 put-back (body mode 0)             133153..133154 (2 rows) after mode 2
P1 put-back (body mode 0)             133473..133474 (2 rows) after mode 2
P1 put-back (body mode 0)             134109..134110 (2 rows) after mode 2
P1 put-back (body mode 0)             134537..134538 (2 rows) after mode 2
P1 put-back (body mode 0)             135352..135353 (2 rows) after mode 2
P1 put-back (body mode 0)             136011..136012 (2 rows) after mode 2
P1 put-back (body mode 0)             136134..136135 (2 rows) after mode 1
P1 put-back (body mode 0)             136715..136716 (2 rows) after mode 2
P1 put-back (body mode 0)             139815..139816 (2 rows) after mode 2
P1 put-back (body mode 0)             140142..140143 (2 rows) after mode 1
P1 lean left (roll min)               128904 (row 1022) -53.0 deg at 85.1, 139116 (row 11234) -47.6 deg at 31.2, 128527 (row 645) -47.5 deg at 81.7
P1 lean right (roll max)              131484 (row 3602) 85.9 deg at 68.9, 131443 (row 3561) 79.4 deg at 71.0, 132590 (row 4708) 69.6 deg at 60.3
P2 reverse                            132128..132152 (25 rows) reverse_l, reverse_r
P2 reverse                            132646..132688 (43 rows) reverse_r
P2 reverse                            138871..139066 (196 rows) reverse_l, reverse_r
P2 backflip clip                      128664..128678 (15 rows) backflip
P2 backflip clip                      128799..128799 (1 rows) backflip
P2 backflip clip                      128943..128949 (7 rows) backflip
P2 backflip clip                      130019..130037 (19 rows) backflip
P2 backflip clip                      138440..138445 (6 rows) backflip
P2 wreck flail                        130419..130537 (119 rows) wreck_air_flail
P2 wreck flail                        130898..131017 (120 rows) wreck_air_flail
P2 wreck flail                        131597..131671 (75 rows) wreck_air_flail
P2 wreck flail                        133802..133927 (126 rows) wreck_air_flail
P2 wreck flail                        134316..134388 (73 rows) wreck_air_flail
P2 wreck flail                        140365..140433 (69 rows) wreck_air_flail
P2 hold (after a wreck)               130538..130620 (83 rows) hold_elbow_left
P2 hold (after a wreck)               131018..131052 (35 rows) hold_elbow_right
P2 hold (after a wreck)               131672..131754 (83 rows) hold_elbow_left
P2 hold (after a wreck)               133928..134010 (83 rows) hold_elbow_right
P2 hold (after a wreck)               134389..134471 (83 rows) hold_elbow_left
P2 hold (after a wreck)               140434..140519 (86 rows) hold_head
P2 put-back (body mode 0)             130621..130622 (2 rows) after mode 2
P2 put-back (body mode 0)             131053..131054 (2 rows) after mode 2
P2 put-back (body mode 0)             131755..131756 (2 rows) after mode 2
P2 put-back (body mode 0)             134011..134012 (2 rows) after mode 2
P2 put-back (body mode 0)             134472..134473 (2 rows) after mode 2
P2 put-back (body mode 0)             138118..138119 (2 rows) after mode 1
P2 put-back (body mode 0)             140520..140521 (2 rows) after mode 2
P2 lean left (roll min)               135846 (row 7964) -65.2 deg at 38.6, 140057 (row 12175) -57.2 deg at 55.8, 130707 (row 2825) -47.3 deg at 59.0
P2 lean right (roll max)              133177 (row 5295) 65.7 deg at 35.1, 135565 (row 7683) 62.0 deg at 53.2, 132865 (row 4983) 56.0 deg at 50.4
