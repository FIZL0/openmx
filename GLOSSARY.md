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
