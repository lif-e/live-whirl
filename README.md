# live-whirl

## UDP preview

The preview stream is sent over UDP to the host(s) specified in the `UDP_HOST`
environment variable. Provide a comma-separated list to broadcast to multiple
machines:

```
UDP_HOST=192.168.1.10,192.168.1.11 make run-headless-video
```

Each host receives the MPEG-TS stream on `UDP_PORT` (default `12345`).