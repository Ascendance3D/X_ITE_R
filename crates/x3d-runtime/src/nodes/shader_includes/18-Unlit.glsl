

#include <Fragment>

uniform x3d_UnlitMaterialParameters x3d_Material;



      #if defined (X3D_EMISSIVE_TEXTURE)

      uniform x3d_EmissiveTextureParameters x3d_EmissiveTexture;

      vec4
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

         vec4 textureColorComponents = textureColor .rgba;

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
      

vec4
getEmissiveColor ()
{
   // Get emissive parameter.

   float alpha = 1.0 - x3d_Material .transparency;

   #if defined (X3D_COLOR_MATERIAL)
      vec4 emissiveColor = vec4 (color .rgb, color .a * alpha);
   #else
      vec4 emissiveColor = vec4 (x3d_Material .emissiveColor, alpha);
   #endif

   // Get texture color.

   #if defined (X3D_EMISSIVE_TEXTURE)
      emissiveColor *= getEmissiveTexture ();
   #elif defined (X3D_TEXTURE)
      emissiveColor = getTextureColor (emissiveColor, vec4 (vec3 (1.0), alpha));
   #endif

   return emissiveColor;
}

vec4
getMaterialColor (const in vec4 fragCoord)
{
   vec4 finalColor = getEmissiveColor ();

   #if defined (X3D_TEXTURE_PROJECTION)
      finalColor .rgb *= getTextureProjectorColor ();
   #endif

   return finalColor;
}
