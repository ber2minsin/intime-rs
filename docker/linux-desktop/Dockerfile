# Headless Sway + grim environment for Linux capture / focus replay tests.
FROM fedora:41

RUN dnf install -y \
      sway grim foot dbus-daemon at-spi2-core \
      mesa-dri-drivers vulkan-loader \
      cargo rust \
    && dnf clean all

WORKDIR /workspace
COPY entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh

ENV WLR_BACKENDS=headless
ENV WLR_LIBINPUT_NO_DEVICES=1
ENV XDG_RUNTIME_DIR=/tmp/runtime-intime
ENV SWAYSOCK=/tmp/sway-ipc.sock

ENTRYPOINT ["/entrypoint.sh"]
