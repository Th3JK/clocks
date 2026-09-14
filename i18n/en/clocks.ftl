app-title = Clocks
about = About
settings = Settings
repository = Repository
report-issue = Report an issue
view = View

# Navigation
nav-world-clocks = World Clocks
nav-stopwatch = Stopwatch
nav-alarm = Alarm
nav-timer = Timer
nav-pomodoro = Pomodoro
nav-chess = Chess Clock
nav-workout = Workout
select-a-view = Select a view

# Common buttons
cancel = Cancel
save = Save
delete = Delete
start = Start
pause = Pause
resume = Resume
reset = Reset
notification-open = Open Clocks
dismiss = Dismiss
snooze = Snooze
add = Add

# Settings
time-format = Time Format
time-format-system = System
time-format-24h = 24-hour
time-format-12h = 12-hour

# Tooltips
tooltip-add = Add
tooltip-edit = Edit
tooltip-delete = Delete
tooltip-remove = Remove
tooltip-start = Start
tooltip-pause = Pause
tooltip-reset = Reset
tooltip-resume = Resume
tooltip-lap = Lap
tooltip-history = History

# Common labels
label = Label
time = Time
repeat = Repeat
duration = Duration
sound = Sound:
minutes-value = { $value } min

# File dialog
choose-sound-file = Choose a sound file

# Notifications
notification-timer-complete = Timer Complete
notification-pomodoro = Pomodoro
notification-alarm = Alarm
notification-alarm-snoozed = Alarm (Snoozed)

# World Clocks
world-clocks-title = World Clocks
local = Local
same-time = Same time
search-timezone = Search timezone...
type-to-search = Type to search for a timezone
type-at-least-2 = Type at least 2 characters
no-timezones-found = No timezones found
timezone-added = { $name } (added)
add-clock = Add Clock
world-clocks-add-button = Add World Clock…
world-clocks-current-timezone = Current timezone
world-clocks-hours-ahead = { $hours } hours ahead
world-clocks-hours-behind = { $hours } hours behind
world-clocks-same-time = Same time
world-clocks-detail-sunrise = Sunrise
world-clocks-detail-sunset = Sunset
world-clocks-no-sun-data = --:--
tooltip-edit-mode = Edit
tooltip-done-editing = Done

# Stopwatch
stopwatch-title = Stopwatch
stop = Stop
lap = Lap
lap-entry = Lap { $id }
fastest = fastest
slowest = slowest
no-history = No history yet
history-hint = Completed stopwatch sessions will appear here
clear-all-history = Clear All History
export-all-history = Export All
export-record = Export
export-csv = Export as CSV
export-success = History exported successfully
export-failure = Export failed
export-filename-all = stopwatch-history.csv
export-filename-record = stopwatch-record.csv
session-label = Session label
total-time = Total: { $time }
laps-recorded = { $count } laps recorded
session-default = Session { $id }
stopwatch-history = Stopwatch History

# Alarms
alarms-title = Alarms
no-alarms = No alarms set
alarm-ringing = Alarm: { $label }
ringing = Ringing...
alarm-label-placeholder = Alarm label
alarm-default-label = Alarm
alarm-snoozed-until = Snoozed until { $time }
once = Once
every-day = Every Day
select-specific-days = Or select specific days:
snooze-duration = Snooze Duration
ring-duration = Ring Duration
new-alarm = New Alarm
edit-alarm = Edit Alarm
create-alarm = Create alarm
alarm-toast-hours-minutes = Alarm in { $hours }h { $minutes }m
alarm-toast-minutes = Alarm in { $minutes }m
alarm-toast-less-than-minute = Alarm in less than a minute

# AM/PM labels
am = AM
pm = PM

# Day names (short)
day-mon = Mon
day-tue = Tue
day-wed = Wed
day-thu = Thu
day-fri = Fri
day-sat = Sat
day-sun = Sun

# Timer
timer-title = Timer
no-timers = No timers set
create-timer = Create timer
timer-label-placeholder = Timer label
timer-default-label = Timer { $id }
repeat-on = Repeat: ON
repeat-off = Repeat: OFF
repeat-count = Count:
infinite-repeats = 0 = infinite repeats
repeat-progress-infinite = Repeat: { $completed } / ∞
repeat-progress = Repeat: { $completed } / { $total }
add-timer = Add Timer
edit-timer = Edit Timer

# Pomodoro
pomodoro-title = Pomodoro
no-pomodoro-timers = No Pomodoro timers
session-info = Session { $number } - { $session_type }
session-work = WORK
session-short-break = SHORT BREAK
session-long-break = LONG BREAK
skip = Skip
progress-info = { $completed } / { $target } sessions | { $focused }m focused
create-pomodoro = Create pomodoro
tooltip-skip = Skip
stat-focus-today = Focus today
stat-streak = Streak
stat-this-week = This week
stat-streak-days = { $count } days
focus-hours-minutes = { $hours }h { $minutes }m
focus-minutes = { $minutes }m
edit-pomodoro = Edit Pomodoro Timer
new-pomodoro = New Pomodoro Timer
label-placeholder-pomodoro = Label (e.g. Study, Work)
default-durations = Default Durations
work-label = Work:
short-break-label = Short Break:
long-break-label = Long Break:
pomodoro-default-label = Pomodoro { $id }
pomodoro-transition = { $label }: { $prev } complete! Starting { $next }
pomodoro-settings = Pomodoro Settings

# Shortcuts
palette-title = Quick action
palette-placeholder = e.g. 5m timer, alarm 6:30, clock tokyo
palette-hint = Type a duration, a time, or a page name
palette-no-match = Nothing matches that
palette-run = Run
palette-preview-timer = Start a { $duration } timer { $label }
palette-preview-alarm = Set an alarm for { $time } { $label }
palette-preview-countdown = Add a countdown to { $date } { $label }
palette-preview-clock = Add a world clock for "{ $query }"
palette-preview-navigate = Go to { $page }
palette-preview-start = Start { $label }
shortcuts = Shortcuts
shortcuts-description = Keyboard shortcuts available in this application
shortcuts-close = Close
shortcuts-global = Global
shortcuts-page = Page
shortcuts-quit = Quit
shortcuts-next-tab = Next tab
shortcuts-prev-tab = Previous tab
shortcuts-tabs = Tabs
shortcuts-show-shortcuts = Show shortcuts
shortcuts-quick-action = Quick action
shortcuts-start-pause = Start / Pause
shortcuts-lap = Lap (Stopwatch)
shortcuts-reset = Reset
shortcuts-new-item = New item
shortcuts-skip-break = Skip break (Pomodoro)
shortcuts-chess-switch = Switch clock (Chess)

# Settings — world clocks section
settings-section-background = Background
settings-background-description = Alarms and timers ring even when Clocks is closed, as long as the background service starts with your session
autostart-enable = Start automatically
autostart-already-enabled = Starts automatically with your session
autostart-enabled = Clocks will now start with your session
autostart-denied = Permission was not granted
autostart-failed = Could not enable autostart: { $error }
autostart-reason = Ring alarms while Clocks is closed
settings-section-sidebar = Sidebar
settings-sidebar-description = Drag to reorder pages, or switch one off to hide it
settings-section-world-clocks = World Clocks
settings-auto-sort-world-clocks = Automatically sort by timezone offset

# Settings — alarms section
settings-section-alarms = Alarms
settings-auto-sort-alarms = Automatically sort alarms by time

# Settings — stopwatch section
settings-section-stopwatch = Stopwatch
settings-auto-clear-stopwatch-history = Automatically clear history after session ends

# Settings — confirmation dialogs section
settings-section-confirmation-dialogs = Confirmation dialogs
settings-confirm-delete-alarm = Confirm before deleting an alarm
settings-confirm-delete-timer = Confirm before deleting a timer
settings-confirm-delete-world-clock = Confirm before deleting a world clock
settings-confirm-delete-pomodoro = Confirm before deleting a pomodoro
settings-confirm-clear-stopwatch = Confirm before clearing stopwatch history

# Confirmation dialog — titles
confirm-delete-alarm-title = Delete alarm?
confirm-delete-timer-title = Delete timer?
confirm-delete-world-clock-title = Delete world clock?
confirm-delete-pomodoro-title = Delete pomodoro?
confirm-clear-stopwatch-title = Clear history?

# Confirmation dialog — body
confirm-delete-alarm-body = This alarm will be permanently removed.
confirm-delete-timer-body = This timer will be permanently removed.
confirm-delete-world-clock-body = This world clock will be permanently removed.
confirm-delete-pomodoro-body = This pomodoro will be permanently removed.
confirm-clear-stopwatch-body = All lap and session history will be permanently cleared.

# Confirmation dialog — shared
confirm-dont-show-again = Don't show again
confirm-button-cancel = Cancel
confirm-button-delete = Delete
confirm-button-clear = Clear

# Shared
seconds-value = { $value } s

# Chess clock
chess-title = Chess Clock
chess-settings = Chess Clock Settings
chess-white = White
chess-black = Black
chess-turn = { $player } to move
chess-paused = Paused
chess-move-number = #
chess-time-control = { $base }+{ $increment }
chess-press-to-start = { $player } to move — press a clock or Space to start
chess-moves = { $moves } moves
chess-moves-last = { $moves } moves · last { $seconds }s
chess-lost-on-time = Lost on time
chess-won = Wins on time
chess-flagged = { $player } ran out of time
chess-winner = { $player } wins on time
chess-presets = Presets
chess-preset-bullet = Bullet 1+0
chess-preset-blitz = Blitz 3+2
chess-preset-rapid = Rapid 10+0
chess-preset-classical = Classical 30+0
chess-base-time = Base time
chess-increment = Increment
chess-apply = Apply & reset
chess-apply-hint = Applying new settings resets the clocks.
notification-chess = Chess Clock

# Workout (HIIT/Tabata)
workout-title = Workout
workout-prep = PREPARE
workout-work = WORK
workout-rest = REST
workout-set-rest = SET BREAK
workout-done = DONE
workout-preset-tabata = Tabata
workout-preset-hiit = HIIT
workout-status-blocks = { $step } · Block { $block }/{ $blocks } · Round { $rep }/{ $reps }
workout-summary = { $blocks } blocks · { $total }
workout-default-label = Workout { $id }
workout-complete = { $label } complete
workout-phase = { $label }: { $phase }
workout-edit = Edit Workout
workout-new = New Workout
workout-label-placeholder = Label (e.g. Morning HIIT)
workout-presets = Presets
workout-prep-label = Prepare
workout-work-label = Work
workout-rest-label = Rest
workout-rounds-label = Rounds
workout-sets-label = Sets
workout-set-rest-label = Set break
workout-add = Add Workout
workout-total-duration = Total { $total }
workout-skip-last-recovery = Skip final recovery
workout-repeat = Repeat
workout-block-n = Block { $n }
workout-add-step = Add step
workout-add-block = Add block
workout-edit-blocks-title = Edit: { $label }
workout-edit-blocks = Edit blocks…
workout-kind-recovery = Recovery
workout-kind-effort = Effort
workout-kind-prep = Prep
create-workout = Create workout
notification-workout = Workout

# Countdown to events
nav-countdown = Countdown
countdown-title = Countdown
countdown-edit = Edit Event
countdown-new = New Event
countdown-add = Add Event
countdown-label-placeholder = Event name (e.g. Launch day)
countdown-pick-date = Pick a date
countdown-default-label = Event { $id }
create-countdown = Create Event
countdown-yearly = Yearly
countdown-yearly-label = Repeat every year
countdown-reminders = Remind me before
countdown-reminder-5min = 5 minutes before
countdown-reminder-15min = 15 minutes before
countdown-reminder-1hour = 1 hour before
countdown-reminder-1day = 1 day before
countdown-reminder-1week = 1 week before
countdown-reminder-count = { $count ->
    [one] { $count } reminder
   *[other] { $count } reminders
}
countdown-reminder-body = { $label } — { $when }
countdown-arrived = { $label } is here
countdown-dhms = { $days }d { $hours }h { $minutes }m { $seconds }s
countdown-ago = { $when } (passed)
countdown-passed = Passed
countdown-edit = Edit event
countdown-days = { $days ->
    [one] { $days } day
   *[other] { $days } days
}
notification-countdown = Countdown
