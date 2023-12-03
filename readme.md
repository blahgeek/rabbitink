# RabbitInk

Use e-ink screens as low-latency computer monitor that is suitable for coding and writing.
(a.k.a., poor man's alternative to e.g. [DASUNG Paperlike monitors](https://shop.dasung.com/products/dasung-e-ink-paperlike-hd-front-light-and-touch-13-3-monitor).

https://github.com/blahgeek/rabbitink/assets/1308450/1b2d3a3b-1c04-49ea-a24c-6e69a91613c8

## Supported hardware & platforms

RabbitInk supports e-ink screens with **IT8915 USB controllers**.
You may buy such products (screen + control board) from [Waveshare](https://www.waveshare.com/product/displays/e-paper.htm)
([chinese](https://www.waveshare.net/list.html?cat=288&sid=MjU5&sid2=NTU=&pno=1)), filter those with USB interface.
*(disclaimer: I'm not affiliated to it)*.
Simply connect the controller to your PC with USB, **no extra hardwares are required**.
I have verified that the following models works:

- Waveshare 6inch HD, 1448x1072 resolution
- Waveshare 13.3inch, 1600x1200 resolution (the is the one in the above video, personally recommended for larger screen size)

**Linux with X11** is the main supported platform. Therotically it also works in macOS and Windows but I have not tested yet.
I'm not sure about Wayland and I don't care.

## Low latency & techniques explained

By default, rabbitink runs in a `mono_bayers4` fast run mode (this is the mode in the above video),
which uses **black-white only color, with bayers dithering**.
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
4. Actually displaying the pixels in screen. This is the slowest part and it depends on the exact waveform used,
which in turn depends on the display mode and current temperature (higher temperature leads to faster updates).
RabbitInk's default run mode uses the fastest A2 display mode, which takes about 170 ms to update the full screen,
or about 137 ms to update only few rows, in about 27 celsius degree room temperature.

In order to reduce ghosting effect caused by the A2 display mode while also keeping the latency
and flickering to minimal, RabbitInk also uses a simple herustic method
to occationally use other display mode:

- when a large portion of the frame is changes, DU display mode would be
used (these are usually the cases that are not very latency sensitive to users any way. e.g. scrolling);
- when the screen has not been updated for a certain time, a GC16 clear is triggered.


Besides from the default `mono_bayers4` run mode, RabbitInk also supports another `gray` run mode that supports
**16 level gray colors with floyd steinberg dithering**. This mode provides much better quality but is not optimized
for low latency. It's suitable for e.g. PDF reading. You can easily switch between run modes while RabbitInk is running.


## How to use

### Quick start

```
$ cargo build --release
$ ./target/release/rabbitink --vcom <vcom value for your eink display, e.g. "2.3">
```

This should mirror your desktop (by screen capturing) to the eink display.
Run `rabbitink --help` for more options.

### Display control while running

- Send `USR1` signal to rabbitink process would force a screen refresh (GC16), to clear ghosting.
- Write run-mode to `/tmp/rabbitink_run_mode.config` followed by a `USR1` signal, to switch rabbitink run-mode.
  Available run-modes are:
  - `mono_bayers4` (default): mono color, bayers 4x4 dithering
  - `mono_bayers2`: mono color, bayers 2x2 dithering
  - `mono_naive`: mono color, no dithering
  - `gray`: 16 level gray color, floyd steinberg dithering
  
I hightly recommend binding above actions to global keyboard shortcuts.

### Configure as secondary monitor

All rabbitink do is basically mirroring using screen capture libraries.
In order to use it like a secondary monitor, you need to configure your system to have an extra virtual monitor
and capture from that virtual monitor instead (see `--source-offx` and `--source-offy` options).

For X11 with nvidia driver, use "ConnectedMonitor" [configuration](https://unix.stackexchange.com/questions/559918/how-to-add-virtual-monitor-with-nvidia-proprietary-driver)
to force enabling one more unused display port, then use `xrandr` to configure the display size to be
the same size as the eink display. For example, in my case, my LCD display (resolution 2560x1440) is connected to the HDMI-0 port and
DP-0 port is used as the virtual display for eink (1600x1200):

```
xrandr --fb 4160x1440 --output HDMI-0 --mode 2560x1440 --primary --pos 1600x0 --output DP-0 --mode 1024x768 --scale-from 1600x1200 --pos 0x0
```

### Application configuration

A proper theme and color scheme in editor/terminal is essential for good user experience.
This varies across different applications and it's out the scope of this documentation, but some general advice:

- *IMPORTANT* Disable font antialias. In linux, use [fontconfig](https://askubuntu.com/questions/396122/disabling-the-anti-aliasing-for-a-specific-font-with-users-fonts-conf).
- Use pure white and black as background and default foreground.
- Reduce font color options. Use italics, weights and underlines instead.
  However, it does NOT means that all colors should be removed:
  it's just they will become "gray-ish" because of dithering.
- Try to have application automatically triggering refresh (by sending `USR1` signal as mentioned above)
  when it knows some context switch happened (e.g. after opening a new file).
  
Here's some of my configurations:
[emacs theme](https://github.com/blahgeek/emacs.d/blob/8263afe9e95839e17ffacd7713030e49bd64b16a/monoink-theme.el),
[emacs config](https://github.com/blahgeek/emacs.d/blob/8263afe9e95839e17ffacd7713030e49bd64b16a/init.el#L342-L362),
[wm config](https://github.com/blahgeek/i3config/blob/076fe53e97cabe8abb86bf0ec65580f74f10ac7d/config#L68-L73).
