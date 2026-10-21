
#include <Fragment>

uniform x3d_MaterialParameters x3d_Material;

#if defined (X3D_LIGHTING)


      #if defined (X3D_AMBIENT_TEXTURE)

      uniform x3d_AmbientTextureParameters x3d_AmbientTexture;

      vec3
      getAmbientTexture ()
      {
         // Get texture color.

         vec3 texCoord = getTexCoord (x3d_AmbientTexture .textureTransformMapping, x3d_AmbientTexture .textureCoordinateMapping);

         #if defined (X3D_AMBIENT_TEXTURE_FLIP_Y)
            // Flip Y if needed. Must be done after.
            texCoord .y = 1.0 - texCoord .y;
         #endif

         #if defined (X3D_AMBIENT_TEXTURE_2D)
            vec4 textureColor = texture (x3d_AmbientTexture .texture2D, texCoord .st);
         #elif defined (X3D_AMBIENT_TEXTURE_3D)
            vec4 textureColor = texture (x3d_AmbientTexture .texture3D, texCoord);
         #elif defined (X3D_AMBIENT_TEXTURE_CUBE)
            vec4 textureColor = texture (x3d_AmbientTexture .textureCube, texCoord);
         #endif

         vec3 textureColorComponents = textureColor .rgb;

         #if defined (X3D_COLORSPACE_SRGB)
            #if defined (X3D_AMBIENT_TEXTURE_LINEAR)
               #if 1
                  textureColorComponents = linearToSRGB (textureColorComponents);
               #endif
            #endif
         #elif defined (X3D_COLORSPACE_LINEAR)
            #if !defined (X3D_AMBIENT_TEXTURE_LINEAR)
               #if 1
                  textureColorComponents = sRGBToLinear (textureColorComponents);
               #endif
            #endif
         #endif

         return textureColorComponents;
      }
      #endif
      

vec3
getAmbientColor (const in vec3 diffuseColor)
{
   // Get ambient parameter.

   vec3 ambientColor = x3d_Material .ambientIntensity * diffuseColor;

   // Get texture color.

   #if defined (X3D_AMBIENT_TEXTURE)
      ambientColor *= getAmbientTexture ();
   #endif

   return ambientColor;
}
#endif



      #if defined (X3D_DIFFUSE_TEXTURE)

      uniform x3d_DiffuseTextureParameters x3d_DiffuseTexture;

      vec4
      getDiffuseTexture ()
      {
         // Get texture color.

         vec3 texCoord = getTexCoord (x3d_DiffuseTexture .textureTransformMapping, x3d_DiffuseTexture .textureCoordinateMapping);

         #if defined (X3D_DIFFUSE_TEXTURE_FLIP_Y)
            // Flip Y if needed. Must be done after.
            texCoord .y = 1.0 - texCoord .y;
         #endif

         #if defined (X3D_DIFFUSE_TEXTURE_2D)
            vec4 textureColor = texture (x3d_DiffuseTexture .texture2D, texCoord .st);
         #elif defined (X3D_DIFFUSE_TEXTURE_3D)
            vec4 textureColor = texture (x3d_DiffuseTexture .texture3D, texCoord);
         #elif defined (X3D_DIFFUSE_TEXTURE_CUBE)
            vec4 textureColor = texture (x3d_DiffuseTexture .textureCube, texCoord);
         #endif

         vec4 textureColorComponents = textureColor .rgba;

         #if defined (X3D_COLORSPACE_SRGB)
            #if defined (X3D_DIFFUSE_TEXTURE_LINEAR)
               #if 1
                  textureColorComponents = linearToSRGB (textureColorComponents);
               #endif
            #endif
         #elif defined (X3D_COLORSPACE_LINEAR)
            #if !defined (X3D_DIFFUSE_TEXTURE_LINEAR)
               #if 1
                  textureColorComponents = sRGBToLinear (textureColorComponents);
               #endif
            #endif
         #endif

         return textureColorComponents;
      }
      #endif
      

vec4
getDiffuseColor ()
{
   // Get diffuse parameter.

   float alpha = 1.0 - x3d_Material .transparency;

   #if defined (X3D_COLOR_MATERIAL)
      vec4 diffuseColor = vec4 (color .rgb, color .a * alpha);
   #else
      vec4 diffuseColor = vec4 (x3d_Material .diffuseColor, alpha);
   #endif

   // Get texture color.

   #if defined (X3D_DIFFUSE_TEXTURE)
      diffuseColor *= getDiffuseTexture ();
   #elif defined (X3D_TEXTURE)
      diffuseColor = getTextureColor (diffuseColor, vec4 (x3d_Material .specularColor, alpha));
   #endif

   return diffuseColor;
}

#if defined (X3D_LIGHTING)


      #if defined (X3D_SPECULAR_TEXTURE)

      uniform x3d_SpecularTextureParameters x3d_SpecularTexture;

      vec3
      getSpecularTexture ()
      {
         // Get texture color.

         vec3 texCoord = getTexCoord (x3d_SpecularTexture .textureTransformMapping, x3d_SpecularTexture .textureCoordinateMapping);

         #if defined (X3D_SPECULAR_TEXTURE_FLIP_Y)
            // Flip Y if needed. Must be done after.
            texCoord .y = 1.0 - texCoord .y;
         #endif

         #if defined (X3D_SPECULAR_TEXTURE_2D)
            vec4 textureColor = texture (x3d_SpecularTexture .texture2D, texCoord .st);
         #elif defined (X3D_SPECULAR_TEXTURE_3D)
            vec4 textureColor = texture (x3d_SpecularTexture .texture3D, texCoord);
         #elif defined (X3D_SPECULAR_TEXTURE_CUBE)
            vec4 textureColor = texture (x3d_SpecularTexture .textureCube, texCoord);
         #endif

         vec3 textureColorComponents = textureColor .rgb;

         #if defined (X3D_COLORSPACE_SRGB)
            #if defined (X3D_SPECULAR_TEXTURE_LINEAR)
               #if 1
                  textureColorComponents = linearToSRGB (textureColorComponents);
               #endif
            #endif
         #elif defined (X3D_COLORSPACE_LINEAR)
            #if !defined (X3D_SPECULAR_TEXTURE_LINEAR)
               #if 1
                  textureColorComponents = sRGBToLinear (textureColorComponents);
               #endif
            #endif
         #endif

         return textureColorComponents;
      }
      #endif
      

vec3
getSpecularColor ()
{
   // Get specular parameter.

   vec3 specularColor = x3d_Material .specularColor;

   // Get texture color.

   #if defined (X3D_SPECULAR_TEXTURE)
      specularColor *= getSpecularTexture ();
   #endif

   return specularColor;
}
#endif



      #if defined (X3D_EMISSIVE_TEXTURE)

      uniform x3d_EmissiveTextureParameters x3d_EmissiveTexture;

      vec3
      getEmissiveTexture ()
      {
         // Get texture color.

         vec3 texCoord = getTexCoord (x3d_EmissiveTexture .textureTransformMapping, x3d_EmissiveTexture .textureCoordinateMapping);

         #if defined (X3D_EMISSIVE_TEXTURE_FLIP_Y)
            // Flip Y if needed. Must be done after.
            texCoord .y = 1.0 - texCoord .y;
         #endif

         #if defined (X3D_EMISSIVE_TEXTURE_2D)
            vec4 textureColor = texture (x3d_EmissiveTexture .texture2D, texCoord .st);
         #elif defined (X3D_EMISSIVE_TEXTURE_3D)
            vec4 textureColor = texture (x3d_EmissiveTexture .texture3D, texCoord);
         #elif defined (X3D_EMISSIVE_TEXTURE_CUBE)
            vec4 textureColor = texture (x3d_EmissiveTexture .textureCube, texCoord);
         #endif

         vec3 textureColorComponents = textureColor .rgb;

         #if defined (X3D_COLORSPACE_SRGB)
            #if defined (X3D_EMISSIVE_TEXTURE_LINEAR)
               #if 1
                  textureColorComponents = linearToSRGB (textureColorComponents);
               #endif
            #endif
         #elif defined (X3D_COLORSPACE_LINEAR)
            #if !defined (X3D_EMISSIVE_TEXTURE_LINEAR)
               #if 1
                  textureColorComponents = sRGBToLinear (textureColorComponents);
               #endif
            #endif
         #endif

         return textureColorComponents;
      }
      #endif
      

vec3
getEmissiveColor ()
{
   // Get emissive parameter.

   vec3 emissiveColor = x3d_Material .emissiveColor;

   // Get texture color.

   #if defined (X3D_EMISSIVE_TEXTURE)
      emissiveColor *= getEmissiveTexture ();
   #endif

   return emissiveColor;
}

#if defined (X3D_LIGHTING)


      #if defined (X3D_SHININESS_TEXTURE)

      uniform x3d_ShininessTextureParameters x3d_ShininessTexture;

      float
      getShininessTexture ()
      {
         // Get texture color.

         vec3 texCoord = getTexCoord (x3d_ShininessTexture .textureTransformMapping, x3d_ShininessTexture .textureCoordinateMapping);

         #if defined (X3D_SHININESS_TEXTURE_FLIP_Y)
            // Flip Y if needed. Must be done after.
            texCoord .y = 1.0 - texCoord .y;
         #endif

         #if defined (X3D_SHININESS_TEXTURE_2D)
            vec4 textureColor = texture (x3d_ShininessTexture .texture2D, texCoord .st);
         #elif defined (X3D_SHININESS_TEXTURE_3D)
            vec4 textureColor = texture (x3d_ShininessTexture .texture3D, texCoord);
         #elif defined (X3D_SHININESS_TEXTURE_CUBE)
            vec4 textureColor = texture (x3d_ShininessTexture .textureCube, texCoord);
         #endif

         float textureColorComponents = textureColor .a;

         #if defined (X3D_COLORSPACE_SRGB)
            #if defined (X3D_SHININESS_TEXTURE_LINEAR)
               #if 0
                  textureColorComponents = linearToSRGB (textureColorComponents);
               #endif
            #endif
         #elif defined (X3D_COLORSPACE_LINEAR)
            #if !defined (X3D_SHININESS_TEXTURE_LINEAR)
               #if 0
                  textureColorComponents = sRGBToLinear (textureColorComponents);
               #endif
            #endif
         #endif

         return textureColorComponents;
      }
      #endif
      

float
getShininessFactor ()
{
   // Get shininess parameter.

   float shininess = x3d_Material .shininess;

   // Get texture color.

   #if defined (X3D_SHININESS_TEXTURE)
      shininess *= getShininessTexture ();
   #endif

   return shininess;
}
#endif



      #if defined (X3D_OCCLUSION_TEXTURE)

      uniform x3d_OcclusionTextureParameters x3d_OcclusionTexture;

      float
      getOcclusionTexture ()
      {
         // Get texture color.

         vec3 texCoord = getTexCoord (x3d_OcclusionTexture .textureTransformMapping, x3d_OcclusionTexture .textureCoordinateMapping);

         #if defined (X3D_OCCLUSION_TEXTURE_FLIP_Y)
            // Flip Y if needed. Must be done after.
            texCoord .y = 1.0 - texCoord .y;
         #endif

         #if defined (X3D_OCCLUSION_TEXTURE_2D)
            vec4 textureColor = texture (x3d_OcclusionTexture .texture2D, texCoord .st);
         #elif defined (X3D_OCCLUSION_TEXTURE_3D)
            vec4 textureColor = texture (x3d_OcclusionTexture .texture3D, texCoord);
         #elif defined (X3D_OCCLUSION_TEXTURE_CUBE)
            vec4 textureColor = texture (x3d_OcclusionTexture .textureCube, texCoord);
         #endif

         float textureColorComponents = textureColor .r;

         #if defined (X3D_COLORSPACE_SRGB)
            #if defined (X3D_OCCLUSION_TEXTURE_LINEAR)
               #if 0
                  textureColorComponents = linearToSRGB (textureColorComponents);
               #endif
            #endif
         #elif defined (X3D_COLORSPACE_LINEAR)
            #if !defined (X3D_OCCLUSION_TEXTURE_LINEAR)
               #if 0
                  textureColorComponents = sRGBToLinear (textureColorComponents);
               #endif
            #endif
         #endif

         return textureColorComponents;
      }
      #endif
      

float
getOcclusionFactor ()
{
   // Get texture color.

   #if defined (X3D_OCCLUSION_TEXTURE)
      return getOcclusionTexture ();
   #else
      return 1.0;
   #endif
}

#if defined (X3D_LIGHTING)

#include <Lighting>
#include <Shadow>

uniform x3d_LightSourceParameters x3d_LightSource [X3D_NUM_LIGHTS];

vec3
getMaterialColor (const in vec3 vertex, const in vec3 N, const in vec3 ambientColor, const in vec3 diffuseColor, const in vec3 specularColor, const in float shininess)
{
   // Apply light sources

   vec3 V = normalize (-vertex);

   vec3 finalColor = vec3 (0.0);

   for (int i = 0; i < X3D_NUM_LIGHTS; ++ i)
   {
      x3d_LightSourceParameters light = x3d_LightSource [i];

      vec3  vL = light .location - vertex;
      float dL = length (light .matrix * vL);

      if (dL <= light .radius || light .radius < 0.0)
      {
         bool di = light .type == x3d_DirectionalLight;
         vec3 d  = light .direction;
         vec3 c  = light .attenuation;
         vec3 L  = di ? -d : normalize (vL);      // Normalized vector from point on geometry to light source i position.
         vec3 H  = normalize (L + V);             // Specular term

         float lightAngle     = max (dot (N, L), 0.0);      // Angle between normal and light ray.
         vec3  diffuseTerm    = diffuseColor * lightAngle;
         float specularFactor = shininess > 0.0 ? pow (max (dot (N, H), 0.0), shininess * 128.0) : 1.0;
         vec3  specularTerm   = specularColor * specularFactor;

         float attenuationFactor     = di ? 1.0 : getAttenuation (c, dL);
         float spotFactor            = light .type == x3d_SpotLight ? getSpotFactor (L, d, light .cutOffAngle, light .beamWidth) : 1.0;
         float attenuationSpotFactor = attenuationFactor * spotFactor;
         vec3  ambientTerm           = light .ambientIntensity * ambientColor;
         vec3  diffuseSpecularTerm   = light .intensity * (diffuseTerm + specularTerm);

         #if defined (X3D_SHADOWS)
            if (lightAngle > 0.0)
               diffuseSpecularTerm = mix (diffuseSpecularTerm, light .shadowColor, getShadowIntensity (i, light));
         #endif

         finalColor += attenuationSpotFactor * light .color * (ambientTerm + diffuseSpecularTerm);
      }
   }

   return finalColor;
}

#endif

#if defined (X3D_GOURAUD_MATERIAL)
// Simulate Gouraud shading. Although this is a Phong shader, we use the same
// calculations as a Gouraud shader would do.
vec4
getMaterialColor (const in vec4 fragCoord)
{
   // Calculate diffuseColor & alpha

   float alpha = 1.0 - x3d_Material .transparency;

   #if defined (X3D_COLOR_MATERIAL)
      vec3 diffuseColor = color .rgb;
      alpha *= color .a;
   #else
      vec3 diffuseColor = x3d_Material .diffuseColor;
   #endif

   #if defined (X3D_LIGHTING)
      vec3  ambientColor  = getAmbientColor (diffuseColor);
      vec3  specularColor = getSpecularColor ();
      float shininess     = getShininessFactor ();
      float normalScale   = x3d_Material .normalScale;

      vec4 finalColor = vec4 (getMaterialColor (vertex, getNormalVector (normalScale), ambientColor, diffuseColor, specularColor, shininess), alpha);
   #else
      vec4 finalColor = vec4 (vec3 (0.0), alpha);
   #endif

   #if defined (X3D_OCCLUSION_TEXTURE)
      finalColor = mix (finalColor, finalColor * getOcclusionFactor (), x3d_Material .occlusionStrength);
   #endif

   finalColor .rgb += getEmissiveColor ();

   #if defined (X3D_DIFFUSE_TEXTURE)
      finalColor *= getDiffuseTexture ();
   #elif defined (X3D_TEXTURE)
      finalColor = getTextureColor (finalColor, vec4 (x3d_Material .specularColor, alpha));
   #endif

   #if defined (X3D_TEXTURE_PROJECTION)
      diffuseColor *= getTextureProjectorColor ();
   #endif

   return finalColor;
}
#endif

#if defined (X3D_PHONG_MATERIAL)
vec4
getMaterialColor (const in vec4 fragCoord)
{
   // Calculate diffuseColor & alpha

   vec4  diffuseColorAlpha = getDiffuseColor ();
   float alpha             = diffuseColorAlpha .a;
   vec3  diffuseColor      = diffuseColorAlpha .rgb;

   #if defined (X3D_TEXTURE_PROJECTION)
      diffuseColor *= getTextureProjectorColor ();
   #endif

   #if defined (X3D_LIGHTING)
      vec3  ambientColor  = getAmbientColor (diffuseColor);
      vec3  specularColor = getSpecularColor ();
      float shininess     = getShininessFactor ();
      float normalScale   = x3d_Material .normalScale;

      vec3 finalColor = getMaterialColor (vertex, getNormalVector (normalScale), ambientColor, diffuseColor, specularColor, shininess);
   #else
      vec3 finalColor = vec3 (0.0);
   #endif

   #if defined (X3D_OCCLUSION_TEXTURE)
      finalColor = mix (finalColor, finalColor * getOcclusionFactor (), x3d_Material .occlusionStrength);
   #endif

   finalColor += getEmissiveColor ();

   return vec4 (finalColor, alpha);
}
#endif
