// The page's <video> elements of the engine's MovieTextures (issue #139).
//
// X_ITE's MovieTexture plays a video file in a <video> element of the page (MovieTexture.loadVideo, setVideo) and takes its
// current picture as the texture. A candidate that does not play (the element's `error` or `abort`) fails, and the next one is
// tried. The engine of X_ITE_R decodes no video: it asks for an element (`load`), and this module makes it, answers when its
// first frame is in or it failed, makes the engine's play, pause, rewind, loop and rate calls on it, and gives the engine
// the pictures and the `currentTime` (WebBrowser.takeVideoOps, videoLoaded, videoFailed, videoFrame).
//
// The picture is read with a 2D canvas (drawImage of the element, getImageData) when it changed. The sound of the element
// plays through the element itself, not through the engine's Web Audio graph (web/API.md).

const EMPTY = new Uint8Array (0);

// MovieTexture.hasAudioTracks.
const hasAudioTracks = video => !! (video .mozHasAudio || video .webkitAudioDecodedByteCount || video .audioTracks ?.length);

export class VideoMirror
{
   #engine;
   #videos = new Map ();      // token -> { video, loaded, dirty, lastTime, wantPlay, retried }
   #canvas = null;
   #context = null;

   // `engine`: () => the WebBrowser of the page.
   constructor (engine)
   {
      this .#engine = engine;
   }

   get size ()
   {
      return this .#videos .size;
   }

   // What WebBrowser.takeVideoOps gave: { live, ops }.
   apply ({ live, ops })
   {
      for (const op of ops)
      {
         const [ name, token, ... args ] = op;

         if (name === "load")
            this .#load (token, args [0]);
         else
            this .#call (name, token, args);
      }

      const keep = new Set (live);

      for (const [ token, entry ] of this .#videos)
      {
         if (keep .has (token))
            continue;

         this .#release (entry);
         this .#videos .delete (token);
      }
   }

   // Before each frame of the engine: the pictures and times of the videos that changed.
   push ()
   {
      const engine = this .#engine ();

      if (!engine)
         return;

      const now = performance .now ();

      for (const [ token, entry ] of this .#videos)
      {
         if (!entry .loaded)
            continue;

         const { video } = entry;

         // The element refused to play (the autoplay policy): try again, as X_ITE's startAudioElement does.
         if (entry .wantPlay && video .paused && !video .ended && now - entry .retried > 500)
         {
            entry .retried = now;
            video .play () .catch (Function .prototype);
         }

         const time = video .currentTime;

         if (!entry .dirty && time === entry .lastTime)
            continue;

         entry .lastTime = time;
         engine .videoFrame (token, time, entry .dirty || video .readyState >= 2 ? this .#pixels (video) : EMPTY);
         entry .dirty = false;
      }
   }

   dispose ()
   {
      for (const entry of this .#videos .values ())
         this .#release (entry);

      this .#videos .clear ();
   }

   // MovieTexture.loadVideo.
   #load (token, url)
   {
      const video = document .createElement ("video");
      const entry = { video, loaded: false, dirty: false, lastTime: -1, wantPlay: false, retried: 0 };

      this .#videos .set (token, entry);

      video .onloadeddata = () =>
      {
         if (this .#videos .get (token) !== entry)
            return;

         entry .loaded = true;
         video .onloadeddata = null;
         video .onerror = video .onabort = null;

         // A new picture each time the element presents one (also after a seek while paused).
         const watch = () =>
         {
            if (this .#videos .get (token) !== entry)
               return;

            entry .dirty = true;
            video .requestVideoFrameCallback (watch);
         };

         video .requestVideoFrameCallback ?.(watch);

         entry .lastTime = video .currentTime;
         this .#engine () ?.videoLoaded (token, video .videoWidth, video .videoHeight, video .duration, hasAudioTracks (video), this .#pixels (video));
      };

      video .onerror =
      video .onabort = event =>
      {
         if (this .#videos .get (token) !== entry || entry .loaded)
            return;

         video .onloadeddata = null;
         video .onerror = video .onabort = null;
         this .#engine () ?.videoFailed (token, event .type);
      };

      video .crossOrigin = "anonymous";
      video .preload     = "auto";
      video .playsInline = true;
      video .src         = url;
   }

   // set_start, set_pause, set_loop and set_speed__ of the engine's MovieTexture.
   #call (name, token, args)
   {
      const entry = this .#videos .get (token);

      if (!entry)
         return;

      const { video } = entry;

      switch (name)
      {
         case "play":
            entry .wantPlay = true;
            entry .retried  = performance .now ();
            video .play () .catch (Function .prototype);
            break;
         case "pause":
            entry .wantPlay = false;
            video .pause ();
            break;
         case "rewind":
            video .currentTime = 0;
            break;
         case "loop":
            video .loop = args [0];
            break;
         case "rate":
            try { video .playbackRate = args [0]; } catch { video .playbackRate = 1; }
            video .preservesPitch = args [1];
            break;
      }
   }

   #release (entry)
   {
      const { video } = entry;

      video .onloadeddata = video .onerror = video .onabort = null;
      video .pause ();
      video .removeAttribute ("src");
      video .load ();
   }

   // The picture of the element as RGBA rows from the top.
   #pixels (video)
   {
      const { videoWidth: width, videoHeight: height } = video;

      if (!this .#canvas)
      {
         this .#canvas  = new OffscreenCanvas (width, height);
         this .#context = this .#canvas .getContext ("2d", { willReadFrequently: true });
      }

      if (this .#canvas .width !== width || this .#canvas .height !== height)
      {
         this .#canvas .width  = width;
         this .#canvas .height = height;
      }

      this .#context .drawImage (video, 0, 0);

      return new Uint8Array (this .#context .getImageData (0, 0, width, height) .data .buffer);
   }
}
