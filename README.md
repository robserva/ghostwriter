## **MAIN IDEA**
> An experiment for the remarkable2 that watches what you write and, when prompted either with a gesture or some on-screen content, can write back to the screen. This is an exploration of various interacts through this handwriting+screen medium.

<img src="docs/simple-chihuahua.jpg" width="300"><img src="docs/chihuahua-logo.png" width="300">

<img src="docs/example-kansas.gif">

## Usage

You need an `OPENAI_API_KEY` environment variable set. I did this by adding it to my ~/.bashrc file:

```sh
# In ~/.bashrc or before you run ghostwriter
export OPENAI_API_KEY=your-key-here
```

Install by copying the binary to your remarkable. Then you have to ssh over and run it, like:

```sh
./ghostwriter --help       # Get the options
./ghostwriter text-assist  # Start a text/keyboard-replying session
```

Draw some stuff on your screen, and then trigger the assistant by *touching/tapping the upper-right corner with your finger*. In the ssh session you'll see other touch-detections and there is a log of what happens while it is processing. You should see some dots drawn during processing and then a typewritten response!

## Status / Journal
* **2024-10-06** - Bootstrapping
  * Basic proof of concept works!!!
  * Drawing back on the screen doesn't work super well; it takes the SVG output from ChatGPT and rasterizes it and then tries to draw lots of individual dots on the screen. The Remarkable flips out a bit ... and when the whole screen is a giant black square it really freaks out and doesn't complete
  * Things that worked at least once:
    * Writing "Fill in the answer to this math problem... 3 + 7 ="
    * "Draw a picture of a chihuahua. Use simple line-art"
* **2024-10-07** - Loops are the stuff of souls
  * I got a rudimentary gesture and status display!
  * So now you can touch in the upper-right and you get an "X" drawn. Then as the input is processed you get further crosses through the X. You have to erase it yourself though :)
* **2024-10-10** - Initial virtual keyboard setup
  * I've started to learn about using the Remarkable with a keyboard, something that I hadn't done before. It's surprisingly limited ... there is basicaly one large textarea for each page with some very basic formatting
  * To write in that I have to make a pretend keyboard, which we can do via rM-input-devices, and I've done basic validation that it works!
  * So now I want to introduce a mode where it always writes back to the text layer and recognizes that text comes from Machine and hadwriting from Human. Not sure that I'll like this mode
* **2024-10-20** - Text output and other modes
  * Slowly starting to rework the code to be less scratch-work, organized a bit
  * Now introduced `./ghostwriter text-assist` mode, uses a virtual keyboard to respond!

## Ideas
* Matt showed me his iOS super calc that just came out, take inspiration from that!
  * This already kinda works, try writing an equation
* A gesture or some content to trigger the request
  * like an x in a certain place
  * or a hover circle -- doesn't need to be an actual touch event per se
* Take a screenshot, feed it into a vision model, get some output, put the output back on the screen somehow
* Like with actual writing; or heck it can draw a million dots on the screen if it does it fast
* OK ... we can also send *keyboard* events! That means we can use the Remarkable text area. This is an awkward and weird text area that lives on a different layer from the drawing
  * So maybe we can say drawing = human, text = machine
  * Probably a lot easier to erase too...
* Prompt library
  * There is already the start of this in <a href="prompts/">prompts/</a>
  * The idea is to give a set of tools (maybe actual llm "tools") that can be configured in the prompt
  * But also could put in there some other things ... like an external command that gets run for the tool
  * Example: a prompt that is good at my todo list management. It would look for "todo", extract that into a todo, and then run `add-todo.sh` or something
    * (which would in turn ssh somewhere to add something to taskwarrior)
* Initial config
  * On first run, maybe create a config file
  * Could prompt for openai key and then write it into the file

## References
* Adapted screen capture from [reSnap](https://github.com/cloudsftp/reSnap)
* Techniques for screen-drawing inspired from [lamp](https://github.com/rmkit-dev/rmkit/blob/master/src/lamp/main.cpy)
* Super cool SVG-to-png done with [resvg](https://github.com/RazrFalcon/resvg)
* Make the keyboard input device even without a keyboard via [rM-input-devices](https://github.com/pl-semiotics/rM-input-devices)

## Development

```sh
# Initial dependencies install (also ... rust)
rustup target add armv7-unknown-linux-gnueabihf
sudo apt-get install gcc-arm-linux-gnueabihf
cargo install cross

# Then to build
cross build --release --target=armv7-unknown-linux-gnueabihf

# And deploy by scp'ing the binary over and run it on the device!
scp target/armv7-unknown-linux-gnueabihf/release/ghostwriter remarkable:
```

## Scratch

I got evtest by getting the ipkg from trotek and untaring it a few levels and then scping it over. Surprisingly it works!

Now I can see that /dev/input/event1 is pen input and /dev/input/event2 is touch input

You can detect distance. The value gets smaller as you get close to the screen with the tip of the pen or eraser

  Event: time 1728139017.789746, type 3 (EV_ABS), code 25 (ABS_DISTANCE), value 105

EV_KEY 320 (BTN_TOOL_PEN) is for pen presence/range
EV_KEY 330 (BTN_TOUCH) is for actual drawing

* https://github.com/rmkit-dev/rmkit/tree/master is great to learn from
* https://github.com/rmkit-dev/rmkit/blob/master/src/lamp/main.cpy -- they've already worked out some other pen-input-drawing! See if we can translate or learn about a reliable way to draw

