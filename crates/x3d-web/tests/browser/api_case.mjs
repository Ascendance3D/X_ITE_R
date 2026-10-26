// The browser API case, run the same way against pinned X_ITE 16.5.0 (oracle/x_ite-web-api.mjs records the
// baseline) and against X_ITE_R (api.test.mjs compares with it). Each phase is a function that Playwright runs in
// the page (`page.evaluate`), so it uses only page globals: `window.X3D_NS` (the X3D namespace of the engine under
// test) and the first <x3d-canvas>. Values are made comparable: no origins, no engine objects.

// Phase 1: set `src`, wait for the world, read the browser, scene, node and field values.
export async function phaseLoad (worldURL)
{
   const X3D     = window .X3D_NS;
   const canvas  = document .querySelector ("x3d-canvas");
   const browser = canvas .browser;
   const out     = { };

   window .caseEvents = [ ];

   for (const name of [ "initialized", "load", "error", "shutdown" ])
      canvas .addEventListener (name, () => window .caseEvents .push (name));

   out .browserIsObject   = typeof browser === "object" && browser !== null;
   out .getBrowserCanvas  = X3D .getBrowser (canvas) === browser;
   out .canvasTag         = Object .prototype .toString .call (canvas);

   const loaded = new Promise ((resolve, reject) =>
   {
      const timeout = setTimeout (() => reject (new Error ("no load event for the world")), 20000);

      canvas .addEventListener ("load", function check ()
      {
         if (browser .currentScene .worldURL !== worldURL)
            return;

         clearTimeout (timeout);
         canvas .removeEventListener ("load", check);
         resolve ();
      });
   });

   canvas .src = worldURL;
   out .srcProperty = canvas .src === worldURL;
   out .srcAttribute = canvas .getAttribute ("src") === worldURL;

   await loaded;

   // The first frames of the world.
   for (let i = 0; i < 3; ++ i)
      await new Promise (requestAnimationFrame);

   const scene    = browser .currentScene;
   const material = scene .getNamedNode ("BoxMaterial");
   const camera   = scene .getNamedNode ("Camera");
   const field    = material .getField ("diffuseColor");

   out .worldURLMatches = scene .worldURL === worldURL;
   out .rootNodes       = Array .from (scene .rootNodes, node => node ? node .getNodeTypeName () : null);
   out .material = {
      typeName:     material .getNodeTypeName (),
      nodeName:     material .getNodeName (),
      diffuseColor: String (material .diffuseColor),
      rgb:          [ material .diffuseColor .r, material .diffuseColor .g, material .diffuseColor .b ],
      transparency: material .transparency,
      fieldType:    field .getTypeName (),
      fieldAccess:  field .getAccessType (),
      fieldString:  String (field),
   };
   out .camera = {
      typeName:    camera .getNodeTypeName (),
      position:    String (camera .position),
      description: camera .description,
      isBound:     camera .isBound,
   };

   // Walk the scene graph through node fields: Transform → Shape → Appearance → Material, Box.
   const transform = scene .rootNodes [1];
   const shape     = transform .children [0];

   out .navigation = {
      children:     transform .children .length,
      rotation:     String (transform .rotation),
      shape:        shape .getNodeTypeName (),
      material:     shape .appearance .material .getNodeName (),
      geometry:     shape .geometry .getNodeTypeName (),
      size:         String (shape .geometry .size),
      solid:        shape .geometry .solid,
   };

   try
   {
      scene .getNamedNode ("NoSuchNode");
      out .missingNode = "no error";
   }
   catch (error)
   {
      out .missingNode = error .message;
   }

   out .events = window .caseEvents .slice ();
   return out;
}

// Phase 2: one field update through the SFNode setter, then an assignment to an outputOnly field.
export async function phaseUpdate ()
{
   const X3D      = window .X3D_NS;
   const scene    = document .querySelector ("x3d-canvas") .browser .currentScene;
   const material = scene .getNamedNode ("BoxMaterial");
   const camera   = scene .getNamedNode ("Camera");
   const out      = { };

   material .diffuseColor = new X3D .SFColor (1, 0, 0);
   out .afterSet = String (material .diffuseColor);

   for (let i = 0; i < 3; ++ i)
      await new Promise (requestAnimationFrame);

   out .afterFrames = String (material .diffuseColor);

   material .transparency = 0.25;
   out .transparency = material .transparency;
   material .transparency = 0;

   // Module code is strict: the proxy's `return false` throws a TypeError.
   try
   {
      camera .isBound = false;
      out .outputOnlyAssign = "no error";
   }
   catch (error)
   {
      out .outputOnlyAssign = error .constructor .name;
   }

   out .isBoundAfter = camera .isBound;

   for (let i = 0; i < 3; ++ i)
      await new Promise (requestAnimationFrame);

   return out;
}

// Phase 3: loadURL failures. The world stays.
export async function phaseFailure (missingURL)
{
   const X3D     = window .X3D_NS;
   const browser = document .querySelector ("x3d-canvas") .browser;
   const before  = browser .currentScene .worldURL;
   const out     = { };

   window .caseEvents .length = 0;

   try
   {
      await browser .loadURL ("not an MFString");
      out .typeCheck = "no error";
   }
   catch (error)
   {
      out .typeCheck = error .message;
   }

   try
   {
      await browser .loadURL (new X3D .MFString (missingURL));
      out .missingWorld = "no error";
   }
   catch (error)
   {
      out .missingWorld = error .message;
   }

   for (let i = 0; i < 3; ++ i)
      await new Promise (requestAnimationFrame);

   out .events = window .caseEvents .slice ();
   out .worldKept = browser .currentScene .worldURL === before;
   out .materialAfter = String (browser .currentScene .getNamedNode ("BoxMaterial") .diffuseColor);
   return out;
}

// Compare two PNG screenshots in the page: the silhouette (pixels that differ from the corner background) and the
// colors inside it. Returns plain numbers; the caller decides.
export async function compareImages ([ a, b ])
{
   const decode = async base64 =>
   {
      const bitmap = await createImageBitmap (await (await fetch (`data:image/png;base64,${base64}`)) .blob ());
      const canvas = new OffscreenCanvas (bitmap .width, bitmap .height);
      const context = canvas .getContext ("2d");

      context .drawImage (bitmap, 0, 0);
      return context .getImageData (0, 0, bitmap .width, bitmap .height);
   };

   const [ A, B ] = await Promise .all ([ decode (a), decode (b) ]);

   if (A .width !== B .width || A .height !== B .height)
      return { sizeA: [ A .width, A .height ], sizeB: [ B .width, B .height ] };

   const background = image => image .data .slice (0, 4);
   const differs    = (d, i, bg) => Math .abs (d [i] - bg [0]) + Math .abs (d [i + 1] - bg [1]) + Math .abs (d [i + 2] - bg [2]) > 24;
   const bgA = background (A), bgB = background (B);

   let inA = 0, inB = 0, both = 0, either = 0, colorDiff = 0, maxDiff = 0;
   const sumA = [ 0, 0, 0 ], sumB = [ 0, 0, 0 ];

   for (let i = 0; i < A .data .length; i += 4)
   {
      const ma = differs (A .data, i, bgA), mb = differs (B .data, i, bgB);

      inA += ma; inB += mb; both += ma && mb; either += ma || mb;

      if (ma && mb)
      {
         for (let c = 0; c < 3; ++ c)
         {
            const d = Math .abs (A .data [i + c] - B .data [i + c]);

            sumA [c] += A .data [i + c];
            sumB [c] += B .data [i + c];
            colorDiff += d;
            maxDiff = Math .max (maxDiff, d);
         }
      }
   }

   return {
      size: [ A .width, A .height ],
      backgroundA: Array .from (bgA),
      backgroundB: Array .from (bgB),
      silhouetteA: inA,
      silhouetteB: inB,
      silhouetteIoU: either ? both / either : 1,
      meanColorA: sumA .map (s => both ? s / both : 0),
      meanColorB: sumB .map (s => both ? s / both : 0),
      meanAbsDiff: both ? colorDiff / (both * 3) : 0,
      maxAbsDiff: maxDiff,
   };
}
