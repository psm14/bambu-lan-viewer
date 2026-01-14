## tl;dr

**connect first, subscribe loosely, read the serial out of the first report, then re-subscribe properly.** no extra user input.

## how it actually works

### step 1: connect to mqtt without knowing the serial

* connect to:

  * host: printer ip
  * port: 8883
  * user: `bblp`
  * pass: lan access code
  * tls: accept self-signed (you’re already doing tofu)

### step 2: subscribe to a wildcard *temporarily*

on initial connect, subscribe to:

```
device/+/report
```

despite earlier lore, **this does work on x1c firmware in lan mode**. the broker will usually allow it *briefly*.

### step 3: parse the first report

the first payload you receive will include identifying info. depending on firmware, one of these shows up:

common patterns people have observed:

```json
{
  "device": {
    "sn": "01S00A123456789"
  }
}
```

or:

```json
{
  "system": {
    "dev_id": "01S00A123456789"
  }
}
```

or sometimes the serial is implied by the topic itself:

```
topic: device/01S00A123456789/report
```

in practice:

* **the topic string is the most reliable**
* split on `/`, grab index 1 → that’s your serial

### step 4: lock it in

once you have the serial:

1. unsubscribe from `device/+/report`
2. subscribe to:

   ```
   device/<serial>/report
   ```
3. store `<serial>` in your `PrinterConfig` (no user involvement)
4. from now on, only use exact-topic subscriptions

### step 5: normal operation

* publish commands to:

  ```
  device/<serial>/request
  ```
* never wildcard again

## why this works (and when it doesn’t)

* the broker is *topic-aware* but not totally locked down
* wildcard subs are usually allowed **until** you start misbehaving
* home assistant, node scripts, and python tools all do this bootstrap trick
* if wildcard sub ever gets blocked in future firmware, you can:

  * fall back to reading serial from bambu studio once
  * or show it as a *read-only detected value* after first connect attempt

## implementation notes (important)

* do the wildcard subscribe **only once per printer**
* if mqtt disconnects before you capture serial, just retry
* don’t persist state until serial is known
* pin tls cert *before* trusting the serial (serial is not a security boundary)

## ux outcome

user inputs only:

* ip
* lan access code

app silently:

* connects
* learns serial
* pins cert
* re-subscribes correctly
* never asks again

this is the least annoying flow and totally normal rn.

