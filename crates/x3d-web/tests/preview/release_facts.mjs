// The "version and build commit shown" check of the release smoke test (smoke.mjs). The expected values come from the
// package's own build-info.json (tools/package-web-preview.sh writes it), not from a fixed version, so the check follows
// the release that is under test. preview.mjs shows `X_ITE_R <browser.version>` in #version and
// `<commit> (<release>, rustc ..., wasm-bindgen ...)` in #build.

const VERSION = /^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$/;
const COMMIT  = /^[0-9a-f]{40}$/;

/** `{ pass, detail }`: the page facts `{ version, build }` against build-info.json `{ version, commit, release }`. */
export function versionFacts (facts, info)
{
   const detail = `${facts ?.version} / ${facts ?.build} (build-info.json: ${info ?.release} ${info ?.version} ${info ?.commit})`;

   if (!info || !VERSION .test (String (info .version)) || !COMMIT .test (String (info .commit)) || typeof info .release !== "string")
      return { pass: false, detail: `build-info.json has no valid version, commit and release; ${detail}` };

   const pass = facts ?.version === `X_ITE_R ${info .version}` &&
      typeof facts ?.build === "string" && facts .build .startsWith (`${info .commit} (${info .release},`);

   return { pass, detail };
}
