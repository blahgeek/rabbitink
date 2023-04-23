# RabbitInk

Use e-ink screens as low-latency computer monitor that is suitable for coding and writing.
(a.k.a., poor man's alternative to e.g. DASUNG Paperlike monitors).

TODO: video

## Hardware requirement & supported platforms

RabbitInk supports e-ink screens with **IT8915 USB controllers**.
You may buy such products (screen + control board) from [Waveshare](https://www.waveshare.com/product/displays/e-paper.htm)
([chinese](https://www.waveshare.net/list.html?cat=288&sid=MjU5&sid2=NTU=&pno=1)), filter those with USB interface.
(disclaimer: I'm not affiliated to it).
I have verified that the following models works:

- Waveshare 6inch HD, 1448x1072 resolution
- Waveshare 13.3inch, 1600x1200 resolution (the one in the above video, personally recommended for larger screen size)

**Linux with X11** is the main supported platform. Therotically it also works in macOS and Windows but I have not tested yet.
I'm not sure about Wayland and I don't care.

## Low latency & technics explained

The default *Mono run mode* of RabbitInk uses **black-white only color, with bayers dithering**.
In this mode, the minimal display latency for a small change (typical on typing) is about **140 ms**.
(The duration is measured to the point when the screen is finished updating the certain change,
but since the updating process is continuous (white -> gray -> black, or black -> gray -> white),
so the actual perceived latency may be even smaller.)

The latency consists of the following parts:

1. Obtaining the frame from OS (screen capture). Using X11 XCB shared memory capturing, this costs about 2.5 ms.
2. Image processing: convert frame to black-white with bayers dithering. This is processed in the GPU
(using [wgpu](https://github.com/gfx-rs/wgpu) for cross-platform support). It costs about 3.5 ms in my desktop.
3. Sending the data to the controller. A packed format is used so that each pixel only take one bit.
The USB 2.0 interface of the IT8915 controller provides about 28MB/s bandwidth, which takes about 8 ms to fully
transmit a frame. To further reduce the latency, only rows that are modified from the last frame are transmitted,
so it typically takes less than 1 ms in typing scenarios.
4. Actually displaying the pixels in screen. This is the lowest part and it depends on the exact waveform used,
which in turn depends on the display mode and current temperature (higher temperature leads to faster updates).
RabbitInk's default run mode uses the fastest A2 display mode, which takes about 170 ms to update the full screen,
or about 137 ms to update only few rows, in about 27 celsius degree room temperature.

In order to reduce ghosting effect caused by the A2 display mode while also keeping the latency
and flickering to minimal, RabbitInk also uses a simple herustic method
to occationally use other display mode:

- when a large portion of the frame is changes, DU display mode would be
used (these are usually the cases that are not very latency sensitive to users any way. e.g. scrolling);
- when the screen has not been updated for a certain time, a GC16 clear is triggered.


Besides from the default *Mono run mode*, RabbitInk also supports another *Gray run mode* that supports
**16 level gray colors with floyd steinberg dithering**. This mode provides much better quality but is not optimized
for low latency. It's suitable for e.g. PDF reading. You can easily switch between run modes while RabbitInk is running.


## How to run

TODO

### 
