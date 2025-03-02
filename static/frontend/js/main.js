'use strict';


const pubstatusDiv = document.getElementById('pubstatus');      
const substatusDiv = document.getElementById('substatus');  
const pubsocket = new WebSocket(`wss://${window.location.host}/publish/test_stream`);
pubsocket.binaryType ="arraybuffer";
pubsocket.onopen = () => {
    pubstatusDiv.textContent = 'Connected';
};
const subsocket = new WebSocket(`wss://${window.location.host}/consume/test_stream`);
subsocket.binaryType ="arraybuffer";
subsocket.onopen = () => {
    substatusDiv.textContent = 'Connected';
};
const mediaSource = new MediaSource();
const callbackQueue = [];
let sourceBuffer;
let mediaRecorder;
let duration;
let isMediaInit = false;

const localVideo = document.querySelector('video#localVideo');
const remoteVideo = document.querySelector('video#remoteVideo');
const streamingBtn = document.querySelector('button#streamingBtn');
streamingBtn.onclick = toggleStreaming;

// navigator.getUserMedia = navigator.getUserMedia ||
//     navigator.webkitGetUserMedia || navigator.mozGetUserMedia || navigator.mediaDevices.getUserMedia;

const constraints = {
    audio: true,
    // video: false
    video: {
        width: { ideal: 1920},
        height: { ideal: 1080 },
        frameRate: { ideal: 60 }
    }
};

navigator.mediaDevices.getUserMedia(constraints).then(successCallback).catch(errorCallback);

mediaSource.addEventListener('sourceopen', function (e) {
    // const mimeCodec = 'audio/webm; codecs=opus';
    const mimeCodec = 'video/webm; codecs="vp8, opus"';
    sourceBuffer = mediaSource.addSourceBuffer(mimeCodec);
    // sourceBuffer.mode = 'segments';
    sourceBuffer.addEventListener('updateend', function () {

        // Update if currentTime is slower than 1 second from the time currently buffered in sourceBuffer
        if (isMediaInit) {
            const ranges = sourceBuffer.buffered;
            const bufferLength = ranges.length;
            if (bufferLength != 0) {
                if (sourceBuffer.buffered.end(0) - remoteVideo.currentTime > 0.5) {
                    remoteVideo.currentTime = sourceBuffer.buffered.end(0);
                    console.log("Update currentTime!!!!");
                }
            }
        } else {
            isMediaInit = true;
        }

        // Append buffer to sourceBuffer if sourceBuffer is not updating 
        if (callbackQueue.length > 0 && !sourceBuffer.updating) {
            sourceBuffer.appendBuffer(callbackQueue.shift());
            console.log('Delayed buffer fix');
        }
    });
}, false);

remoteVideo.src = window.URL.createObjectURL(mediaSource);

subsocket.onmessage = (event) =>{
    if (mediaSource.readyState == 'open') {
        const arrayBuffer = new Uint8Array(event.data);
        if (!sourceBuffer.updating && callbackQueue.length == 0) {
            sourceBuffer.appendBuffer(arrayBuffer);
        } else {
            callbackQueue.push(arrayBuffer);
        }
    }
};

function eventTest(event) {
    console.log('event Test', event);
}

function successCallback(stream) {
    console.log('getUserMedia() got stream: ', stream);
    stream.inactive = eventTest;
    window.stream = stream;
    localVideo.srcObject = stream;
    localVideo.onloadedmetadata = function (event) {
        console.log("onloadedmetadata", event);
    }
    localVideo.addEventListener('play', (event) => {
        console.log("play", event);
    });
}

function errorCallback(error) {
    console.log('navigator.getUserMedia error: ', error);
}

function handleDataAvailable(event) {
    if (event.data && event.data.size > 0) {
        pubsocket.send(event.data);
    }
}

function handleStop(event) {
    console.log('Recorder stopped: ', event);
}

function toggleStreaming() {
    if (streamingBtn.textContent === 'Start Streaming') {
        startStreaming();
    } else {
        stopStreaming();
        streamingBtn.textContent = 'Start Streaming';
    }
}

function startStreaming() {
    // const options = { mimeType: 'audio/webm; codecs=opus' };
    // const options = { mimeType: 'video/webm; codecs="vp8, opus"' };
    const options = {
        audioBitsPerSecond: 128000,
        videoBitsPerSecond: 7500000,
        mimeType: 'video/webm; codecs="vp8, opus"',
      };
    try {
        mediaRecorder = new MediaRecorder(window.stream, options);
    } catch (e0) {
        console.log('Unable to create MediaRecorder with options Object: ', e0);
        try {
            options = { mimeType: 'video/webm,codecs=vp8', bitsPerSecond: 100000 };
            mediaRecorder = new MediaRecorder(window.stream, options);
        } catch (e1) {
            console.log('Unable to create MediaRecorder with options Object: ', e1);
            try {
                options = 'video/vp8'; // Chrome 47
                mediaRecorder = new MediaRecorder(window.stream, options);
            } catch (e2) {
                alert('MediaRecorder is not supported by this browser.\n\n' +
                    'Try Firefox 29 or later, or Chrome 47 or later, with Enable experimental Web Platform features enabled from chrome://flags.');
                console.error('Exception while creating MediaRecorder:', e2);
                return;
            }
        }
    }
    console.log('Created MediaRecorder', mediaRecorder, 'with options', options);
    streamingBtn.textContent = 'Stop Streaming';
    mediaRecorder.onstop = handleStop;
    mediaRecorder.ondataavailable = handleDataAvailable;
    mediaRecorder.start(1); // time slice 1ms
    console.log('MediaRecorder started', mediaRecorder);
}

function stopStreaming() {
    socket.disconnect();
    mediaRecorder.stop();
}