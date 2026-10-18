attribute vec4 x3d_Vertex;
attribute vec3 x3d_Normal;
attribute vec4 x3d_TexCoord0;
attribute vec4 x3d_Color;
uniform mat4 x3d_ProjectionMatrix;
uniform mat4 x3d_ModelViewMatrix;
uniform mat3 x3d_NormalMatrix;
varying vec3 vN;
varying vec3 vP;
varying vec2 vT;
varying vec4 vC;
void main()
{
   vec4 p = x3d_ModelViewMatrix * x3d_Vertex;
   vP = p.xyz;
   vN = normalize (x3d_NormalMatrix * x3d_Normal);
   vT = x3d_TexCoord0.st;
   vC = x3d_Color;
   gl_Position = x3d_ProjectionMatrix * p;
}
