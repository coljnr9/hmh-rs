/* Learning example: two seconds of a quiet 440 Hz tone. */
#define _POSIX_C_SOURCE 200809L
#include <alsa/asoundlib.h>
#include <errno.h>
#include <math.h>
#include <stdint.h>
#include <stdio.h>

int main(int argc, char **argv) {
    const unsigned rate = 48000, channels = 2;
    const unsigned total = rate * 2;
    snd_pcm_t *pcm = NULL;
    const char *device = argc > 1 ? argv[1] : "default";
    int err = snd_pcm_open(&pcm, device, SND_PCM_STREAM_PLAYBACK, 0);
    if (err < 0) {
        fprintf(stderr, "open: %s\n", snd_strerror(err));
        return 1;
    }
    /* Latency is in microseconds; this helper also prepares the stream. */
    err = snd_pcm_set_params(pcm, SND_PCM_FORMAT_S16_LE,
                            SND_PCM_ACCESS_RW_INTERLEAVED,
                            channels, rate, 1, 100000);
    if (err < 0) goto fail;

    for (unsigned frame = 0; frame < total;) {
        /* Explicit little-endian bytes, independent of host endianness. */
        unsigned char samples[256 * 2 * 2];
        unsigned count = total - frame < 256 ? total - frame : 256;
        for (unsigned i = 0; i < count; ++i) {
            int16_t sample = (int16_t)(1200 * sin(2 * 3.141592653589793 *
                                               440 * (frame + i) / rate));
            for (unsigned ch = 0; ch < channels; ++ch) {
                unsigned offset = (i * channels + ch) * 2;
                samples[offset] = (uint16_t)sample & 0xff;
                samples[offset + 1] = (uint16_t)sample >> 8;
            }
        }
        unsigned sent = 0;
        while (sent < count) {
            snd_pcm_sframes_t n = snd_pcm_writei(pcm, samples + sent * channels * 2,
                                                 count - sent);
            if (n == -EAGAIN || n == 0) {
                err = snd_pcm_wait(pcm, 1000);
                if (err < 0) {
                    err = snd_pcm_recover(pcm, err, 1);
                    if (err < 0) goto fail;
                }
                continue;
            }
            if (n < 0) {
                err = snd_pcm_recover(pcm, (int)n, 1);
                if (err < 0) goto fail;
                continue;
            }
            sent += (unsigned)n; /* Return values count frames, not bytes. */
        }
        frame += count;
    }
    err = snd_pcm_drain(pcm); /* Finish queued audio instead of dropping it. */
    if (err < 0) goto fail;
    err = snd_pcm_close(pcm);
    if (err < 0) {
        fprintf(stderr, "close: %s\n", snd_strerror(err));
        return 1;
    }
    return 0;
fail:
    fprintf(stderr, "PCM: %s\n", snd_strerror(err));
    snd_pcm_close(pcm);
    return 1;
}
