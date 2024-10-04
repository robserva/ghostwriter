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