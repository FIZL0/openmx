# s1 glossary

The words for each thing in s1: what the person says, what the app shows and what the code calls it, with
a plain definition and the words to avoid. Kept by `agent-runner glossary` (add, show, forget); no implementation
detail. (Format after mattpocock/skills domain-modeling, MIT.)

## Language

**Auto Photos**:
Photo mode pictures taken on their own at a wreck, a landed trick or the race results screen, saved in the photos folder
_On screen_: Auto Photos (Settings, Display)
_In code_: photo::Auto, menu::AutoPhotos, --auto-photos

**odometer**:
Feet a race card's bike has ridden forward since the grid placed it, counted by its statistics tracker; the results' stats commit counts it as feet ridden
_In code_: race::Tracker::odometer

**race clock**:
The score manager's clock: seconds since the tick after the race's drop, standing once the tip's popup is up; the stats commit reads it as time played and raced
_In code_: race::Race::clock

**outro scene**:
The short tune that plays when a race ends, picked by the best place a player took; the tip popup comes up 2.05 s before it ends, or 7 s on when it has nothing to play
_In code_: race::Supercross::outros, race::Race::stage, race_start::outro_lengths
_Avoid_: race-end ditty

**Safe spot**:
The last place a driven car stood on level, clear ground: where a pause-menu Restart puts that car back, instead of its bay.
_In code_: car::Store::record, InPlay::record
_Avoid_: checkpoint, respawn point
