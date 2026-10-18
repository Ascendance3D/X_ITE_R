precision mediump float;
varying vec3 vN;
#define BRIGHT 0.9
#ifdef X3D_FRAGMENT_SHADER
#define BASE vec3 (0.2, 0.4, BRIGHT)
#else
#define BASE vec3 (1.0, 0.0, 0.0)
#endif
#if defined (X3D_FOG) && x3d_MaxTextures == 4
const float K = 1.0;
#else
const float K = 0.0;
#endif
void main() { gl_FragColor = vec4 (BASE * (0.3 + 0.7 * abs (vN.z)) * K, 1.0); }
