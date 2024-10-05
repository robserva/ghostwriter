## **MAIN IDEA**
A thing for my remarkable2 that watches what I write and, perhaps prompted either with a gesture or some on-screen content, can write on the screen

## Ideas
* Matt showed me his iOS super calc that just came out, take inspiration from that!
* A gesture or some content to trigger the request
  * like an x in a certain place
  * or a hover circle -- doesn't need to be an actual touch event per se
* Take a screenshot, feed it into a vision model, get some output, put the output back on the screen somehow
* Like with actual writing; or heck it can draw a million dots on the screen if it does it fast

## Notes
* You can read and write touch input!
  * `cat /dev/input/touchscreen0 > /tmp/touch-record` will record input
  * Draw something then hit `^C`
  * Now reverse that with `cat /tmp/touch-record > /dev/input/touchscreen0`
  * And it will draw on the screen!!!!!
* You can get a screenshot with reSnap.sh
  * This does the crazy read into memory to grab the screenbuffer
  * We can translate this into another language and then maybe change it to a jpg in ram
  * Tantilizing!!!


## Scratch

rustup target add armv7-unknown-linux-gnueabihf
sudo apt-get install gcc-arm-linux-gnueabihf

cargo build --release --target=armv7-unknown-linux-gnueabihf


cargo install cross
cross build --release --target=armv7-unknown-linux-gnueabihf

scp target/armv7-unknown-linux-gnueabihf/release/ghostwriter remarkable:


I got evtest by getting the ipkg from trotek and untaring it a few levels and then scping it over. Surprisingly it works!

Now I can see that /dev/input/event1 is pen input and /dev/input/event2 is touch input

You can detect distance. The value gets smaller as you get close to the screen with the tip of the pen or eraser

  Event: time 1728139017.789746, type 3 (EV_ABS), code 25 (ABS_DISTANCE), value 105

EV_KEY 320 (BTN_TOOL_PEN) is for pen presence/range
EV_KEY 330 (BTN_TOUCH) is for actual drawing
