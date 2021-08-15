# http-gpio

## Download the binaries

You can download pre-compiled linux ARM binaries from the github action results of this repository:
https://github.com/lovasoa/http-gpio/actions

## Usage

## Launch the server

```
./http-gpio
```

The server listens on port 3030

### List available devices

```shell
curl localhost:3030/gpio/
```

### List available devices

```shell
curl localhost:3030/gpio/
```

```json
[
  {
    "name": "gpiochip0",
    "label": "INT34BB:00",
    "num_lines": 312
  }
]
```

### List available pins in device

```shell
curl localhost:3030/gpio/gpiochip0
```

```json5
[
  {
    "currently_used_by": null,
    // Who is currently using the pin
    "is_active_low": false,
    "is_kernel": false,
    "is_output": false,
    "is_used": false,
    "name": null,
    // Null when the pin is not named (most of the time)
    "offset": 0
  },
  // [...] (One object per pin in device)
]
```

### List characteristics of a single pin

```shell
curl localhost:3030/gpio/gpiochip0/13
```

```json5
{
  "currently_used_by": "http-gpio",
  "is_active_low": false,
  "is_kernel": true,
  "is_output": true,
  "is_used": true,
  "name": null,
  "offset": 13
}
```

### **Read a pin**

```shell
curl localhost:3030/gpio/gpiochip0/13/value
```

```json5
1 // 1 for high, 0 for low
```

### **Set a pin value**

```shell
curl -X POST localhost:3030/gpio/gpiochip0/13/value --data "1"
```

Returns a 200 status code if the pin was successfully set

### Make a pin blink

You can upload a short schedule of values to make a pin alternate between high and low states

```shell
curl -X POST localhost:3030/gpio/gpiochip0/13/value  \
  --data "[500,1000,300,200]"
```

This will set pin 13 
 - to 0 for 500 milliseconds, then
 - to 1 for 1 second (1000 milliseconds), then
 - to 0 for 300 milliseconds
 - to 1 for 200 milliseconds

Then the request will return but the pin will stay set to 1.

Returns a 200 status code if the schedule was successfully applied.


# Original library

See https://www.thirtythreeforty.net/posts/2020/05/mastering-embedded-linux-part-5-platform-daemons/