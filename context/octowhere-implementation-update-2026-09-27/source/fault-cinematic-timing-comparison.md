# Fault exit timing against the Marathon cinematic

Reference: the locally provided 3840 × 2160 Marathon alpha intro cinematic, approximately 29.97 fps. This comparison uses decoded frame timestamps, not the on-screen 1:14–1:15 shorthand. The video is **not included** in this study. The V8 study is 30 fps.

The last substantially intact red cinematic frame is at **76.910 s**, used as time zero below. A sampled study frame begins at `frame / 30` seconds; the review board labels the **end** of each frame, so its displayed time is about 33 ms later than the onset shown here.

| Landmark | Cinematic | Relative to 76.910 s | V8 study | Difference |
| --- | ---: | ---: | ---: | --- |
| Last intact frame / start | 76.910 s | 0 ms | frame 0, 0 ms | aligned |
| First lower-edge disruption | 76.944 s | 34 ms | frame 1, 33 ms | about aligned |
| Large red-field dropout | 76.977 s | 67 ms | frame 2, 67 ms | onset and red area now close |
| New blue scene elements become visible under departing fault graphics | about 77.244 s | about 334 ms | no equivalent until clock fixture at 600 ms | V8 has no scene overlap |
| Last visible fault red | 77.310 s | 400 ms | frame 16, 530 ms | V8 tail about 130 ms longer |
| First frame without fault red | 77.344 s | 434 ms | frame 17, 560 ms | V8 about 126 ms longer |
| Destination fixture begins | distinct blue content by about 77.244 s | about 334 ms | clock fixture at 600 ms | different scenes; V8 is about 266 ms later as a handoff cue |

A coarse red-pixel count at **+67 ms** leaves about **35%** of the red pixels from the cinematic's 76.910 s frame. V8 also leaves about **35%** of its starting red pixels at frame 2, with the same V7 block fragments and central ticker/lettering. The counts are normalized within each composition and are only a visual proxy: the cinematic is widescreen, compressed footage with multiple layers; V8 is a circular device render. At +300 ms the remaining red shares are approximately **15% in the cinematic** and **32% in V8**; the ease-in keeps the surviving area visible much longer before accelerating. V8 lets its last fragments persist until 560 ms.

The **initial loss has similar timing and scale**, while V8's ease-in is considerably slower through the middle and then clears quickly at the end. The cinematic's outgoing fault graphics overlap with incoming blue scene elements. V8 deliberately keeps the complete four-second fault hold, gives the dropout two early steps, and uses a 600 ms separate exit with quadratic ease-in for the surviving portion. `easing-comparison-board.png` compares V7 and V8 at matched times. A later timing pass could compress the exit toward roughly **430–450 ms** and explore how the device clock enters beneath outgoing fault fragments. Those are not changes to the handed-off backup.
