
let frameCounter = 0;
let encoder, settings, interval, userMedia, websocket;
const init = {
    output: handleChunk,
    error: (e) => {
        console.log(e.message);
    },
};
self.onmessage = async function(e) {
    if (e.data.type === 'init') {
        encoder = new VideoEncoder(init);
        encoder.configure(e.data.encoder_config);
        settings = e.data.settings;
        interval = e.data.interval;
        websocket = new WebSocket(e.data.websocketUrl);
        websocket.binaryType = "arraybuffer";
        websocket.onopen = () => {
            console.log("WebSocket connected");
        };
        postMessage({ type: 'ready' });
        // startEncoding();
    }
    if (e.data.type === 'frame') {
        const bitmap = e.data.bitmap;
        var frame = new VideoFrame(bitmap, { timestamp: performance.now() });
        if (encoder.encodeQueueSize > 2) {
            // Too many frames in flight, encoder is overwhelmed
            // let's drop this frame.
            frame.close();
        } else {
            frameCounter++;
            const keyFrame = frameCounter % 60 == 0;
            encoder.encode(frame, { keyFrame });
            frame.close();
        }
    }
};
function handleChunk(chunk, metadata) {
    if (metadata.decoderConfig) {
        console.log("Decoder config:", metadata.decoderConfig);
        // websocket.send(JSON.stringify(metadata.decoderConfig));
    }

    const chunkData = new ArrayBuffer(chunk.byteLength);
    chunk.copyTo(chunkData);
    websocket.send(chunkData);
}
function startEncoding() {
    setInterval(async () => {
        const bitmap = await createImageBitmap(videoPreview, 0, 0, settings.width, settings.height);
        var frame = new VideoFrame(bitmap, { timestamp: performance.now() });
        if (encoder.encodeQueueSize > 2) {
            // Too many frames in flight, encoder is overwhelmed
            // let's drop this frame.
            frame.close();
        } else {
            frameCounter++;
            const keyFrame = frameCounter % 60 == 0;
            encoder.encode(frame, { keyFrame });
            frame.close();
        }
    }, interval);
}