# CT116 GPU passthrough

The production configuration verified on 2026-09-01 gives CT116 access to the
host-owned AMD Radeon RX 5600 XT through `amdgpu`. The VGA function
`0000:03:00.0` uses `amdgpu`; it is not globally bound to `vfio-pci`.
`amdgpu` is loaded at host boot through `/etc/modules-load.d/amdgpu.conf`, and
the active VFIO configuration does not list the AMD VGA/audio PCI IDs or add a
`softdep` that places `vfio-pci` before `amdgpu`.

[`ct116-gpu.conf`](ct116-gpu.conf) records the relevant deployed LXC settings.
Both observed KFD majors, 509 and 511, are allowed because the allocated major
can differ across boots. The hook waits for the actual character devices and
logs the observed KFD major; it does not require a fixed value. The deployed
hook path is `/var/lib/vz/snippets/jarvis-ollama-gpu-passthrough-hook.sh`.

After changing host boot/module configuration, the initramfs must be regenerated
before the next boot. This repository does not claim host-reboot persistence:
only CT restart persistence and the live GPU path have been verified. A complete
Proxmox host reboot requires a coordinated downtime window.

Production evidence captured on 2026-09-01:

- `/dev/kfd`, `/dev/dri/card1`, and `/dev/dri/renderD129` were character devices;
- KFD used major 509 and DRM used major 226;
- Vulkan loaded all 17 model layers on the AMD GPU;
- the controlled inference benchmark generated 165 tokens/s.

VM110 also declares the same physical GPU as `hostpci0`. CT116 is the default
consumer, and VM110 must not start until CT116 has stopped.
