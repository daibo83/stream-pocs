
self.onmessage = async ({data: {track}}) => {
    console.log(track);
    const trackProcessor = new MediaStreamTrackProcessor({track});
    const reader = trackProcessor.readable.getReader();
    while (true) {
        const result = await reader.read();
        if (result.done) break;
        const frame = result.value;
        self.postMessage(frame);
    }
};