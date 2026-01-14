below is a **practical mqtt protocol guide** for bambu x1/p1/a-series in **lan / developer mode**, distilled from community reverse-engineering + real payloads people have captured. this is not “official” and fields can drift with firmware, so treat unknown keys as forward-compatible noise.

---

# bambu lan mqtt protocol (field guide)

## 0. transport recap

* protocol: **mqtt over tls**
* broker: printer ip
* port: **8883**
* tls: **self-signed**
* auth:

  * username: `bblp`
  * password: **lan access code**
* qos: usually `0` or `1` (status often qos0)

---

## 1. topic layout (canonical)

topics are namespaced by **printer serial number** (device id).

### subscribe (status / reports)

```
device/<serial>/report
```

this is the **main firehose**. almost everything you care about arrives here.

### publish (commands / requests)

```
device/<serial>/request
```

you send json blobs here.

> some firmwares also expose:

```
device/<serial>/status
device/<serial>/event
```

but in practice `report` is the one you need.

---

## 2. message envelope

### report message (incoming)

json object, loosely structured, sparse, and **delta-based** (fields appear only when changed).

example (heavily simplified):

```json
{
  "print": {
    "gcode_state": "RUNNING",
    "mc_percent": 42,
    "layer_num": 123,
    "total_layer_num": 300
  },
  "temp": {
    "nozzle_temper": 215.3,
    "bed_temper": 60.0,
    "chamber_temper": 38.2
  },
  "lights_report": {
    "chamber_light": 1
  },
  "time": 1736112345
}
```

### request message (outgoing)

also json. you usually send **one command per message**.

example:

```json
{
  "user_id": "1",
  "print": {
    "sequence_id": "101",
    "command": "pause"
  }
}
```

notes (observed on newer firmware):

* many control commands **require a top-level** `user_id` (string)
* `sequence_id` is typically required and should be a **string**

---

## 3. job / print state

### gcode_state (most important)

found under `print.gcode_state`.

common values observed:

* `IDLE`
* `RUNNING`
* `PAUSE`
* `FINISH`
* `FAILED`
* `STOPPED`

map these to your app’s job state.

### progress

* `print.mc_percent` → integer percent (0–100)
* sometimes also:

  * `layer_num`
  * `total_layer_num`

progress is not guaranteed to be present every report.

---

## 4. temperatures

under `temp` (names are not consistent across firmwares, so be defensive):

* `nozzle_temper` (current)
* `bed_temper`
* `chamber_temper`

sometimes you’ll also see target temps:

* `nozzle_target_temper`
* `bed_target_temper`

phase 1: read current temps only.

---

## 5. light state

reported under:

```json
"print": {
  "lights_report": [
    { "node": "chamber_light", "mode": "on" | "off" }
  ]
}
```

### set light command

publish:

```json
{
  "user_id": "1",
  "system": {
    "sequence_id": "1",
    "command": "ledctrl",
    "led_node": "chamber_light",
    "led_mode": "on",
    "led_on_time": 500,
    "led_off_time": 500,
    "loop_times": 0,
    "interval_time": 0
  }
}
```

(use `"off"` to turn the light off)

note: some firmwares reject `ledctrl` without the top-level `user_id` and timing fields.

---

## 6. pause / resume / stop (core controls)

### pause

```json
{
  "user_id": "1",
  "print": {
    "sequence_id": "101",
    "command": "pause"
  }
}
```

### resume

```json
{
  "user_id": "1",
  "print": {
    "sequence_id": "102",
    "command": "resume"
  }
}
```

### stop

```json
{
  "user_id": "1",
  "print": {
    "sequence_id": "103",
    "command": "stop"
  }
}
```

notes (observed on newer firmware):

* commands are **fire-and-forget**
* success is confirmed only when `gcode_state` changes
* do **not** assume success just because publish returned ok
* printers may also emit a `system` echo with `result: success|fail` and `reason`

---

## 7. timing & cadence

* report messages arrive:

  * frequently while printing (multiple per second)
  * sparsely while idle
* treat reports as **partial updates**
* always merge into last known state

best practice:

* keep a `lastUpdate` timestamp
* consider state “stale” if no report for N seconds

---

## 8. error handling patterns

### command timeout

pattern:

1. send command
2. record “pending command”
3. wait for matching state transition:

   * pause → `gcode_state == PAUSE`
   * resume → `RUNNING`
   * stop → `IDLE` / `STOPPED`
4. if not observed within ~3–5s:

   * clear pending
   * surface error (“command not acknowledged”)

### printer reboot / disconnect

you’ll see:

* mqtt disconnect
* then reconnect
* then a fresh burst of report fields

on reconnect:

* do **not** clear state immediately
* wait for first report, then reconcile

---

## 9. fields you should ignore (for now)

you will see lots of extra stuff:

* ams status
* filament metadata
* camera status
* cloud flags
* calibration info

ignore anything you don’t explicitly model; firmware updates add/remove fields often.

---

## 10. protocol hygiene (important)

* **never assume field presence**
* **never assume full snapshots**
* **never key logic on exact payload equality**
* treat mqtt report as a **stream of patches**

a good internal pattern:

```swift
struct PrinterStatePatch {
  var jobState: JobState?
  var progress01: Double?
  var nozzleC: Double?
  var bedC: Double?
  var chamberC: Double?
  var lightOn: Bool?
}
```

---

## 11. testing tips

* capture real payloads using:

  * mosquitto_sub (with tls verify disabled)
  * home assistant bambu integration debug logs
* build json fixtures from **real traffic**, not guessed schemas
* expect firmware drift; write tolerant decoders

---

## 12. reality check

there is **no stable official schema**. what exists is:

* community observation
* convergence across ha integrations + scripts
* relatively stable core commands (pause/resume/stop/light)

the good news: **pause/resume/stop/light have been rock-solid** across multiple firmware generations.

---

## 13. known working payloads (observed on X1C, Jan 2026)

### ledctrl (chamber light off)

request:

```json
{
  "user_id": "1",
  "system": {
    "sequence_id": "1",
    "command": "ledctrl",
    "led_node": "chamber_light",
    "led_mode": "off",
    "led_on_time": 500,
    "led_off_time": 500,
    "loop_times": 0,
    "interval_time": 0
  }
}
```

response:

```json
{
  "system": {
    "command": "ledctrl",
    "interval_time": 0,
    "led_mode": "off",
    "led_node": "chamber_light",
    "led_off_time": 500,
    "led_on_time": 500,
    "loop_times": 0,
    "reason": "",
    "result": "success",
    "sequence_id": "1"
  }
}
```
