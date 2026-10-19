

// Original source code from:
// https://github.com/KhronosGroup/glTF-Sample-Renderer/blob/main/source/Renderer/shaders/material_info.glsl

struct MaterialInfo
{
   float ior;
   float perceptualRoughness; // roughness value, as authored by the model creator (input to shader)
   vec3 f0_dielectric; // full reflectance color (n incidence angle)

   float alphaRoughness; // roughness mapped to a more linear change in the roughness (proposed by [2])

   float fresnel_w;

   vec3 f90; // reflectance color at grazing angle
   vec3 f90_dielectric;
   float metallic;

   vec3 baseColor;

   // KHR_materials_sheen
   float sheenRoughnessFactor;
   vec3 sheenColorFactor;

   // KHR_materials_clearcoat
   vec3 clearcoatF0;
   vec3 clearcoatF90;
   float clearcoatFactor;
   vec3 clearcoatNormal;
   float clearcoatRoughness;

   // KHR_materials_specular
   float specularWeight; // product of specularFactor and specularTexture.a

   // KHR_materials_transmission
   float transmissionFactor;

   // KHR_materials_volume
   float thickness;
   vec3 attenuationColor;
   float attenuationDistance;

   // KHR_materials_iridescence
   float iridescenceFactor;
   float iridescenceIor;
   float iridescenceThickness;

   // KHR_materials_diffuse_transmission
   float diffuseTransmissionFactor;
   vec3 diffuseTransmissionColorFactor;

   // KHR_materials_anisotropy
   vec3 anisotropicT;
   vec3 anisotropicB;
   float anisotropyStrength;

   // KHR_materials_dispersion
   float dispersion;

   // KHR_materials_volume_scatter
   vec3 multiscatterColor;

   // KHR_materials_retroreflection
   float retroreflectionFactor;
};

#if defined (X3D_MATERIAL_METALLIC_ROUGHNESS)



      #if defined (X3D_BASE_TEXTURE)

      uniform x3d_BaseTextureParameters x3d_BaseTexture;

      vec4
      getBaseTexture ()
      {
         // Get texture color.

         vec3 texCoord = getTexCoord (x3d_BaseTexture .textureTransformMapping, x3d_BaseTexture .textureCoordinateMapping);

         #if defined (X3D_BASE_TEXTURE_FLIP_Y)
            // Flip Y if needed. Must be done after.
            texCoord .y = 1.0 - texCoord .y;
         #endif

         #if defined (X3D_BASE_TEXTURE_2D)
            vec4 textureColor = texture (x3d_BaseTexture .texture2D, texCoord .st);
         #elif defined (X3D_BASE_TEXTURE_3D)
            vec4 textureColor = texture (x3d_BaseTexture .texture3D, texCoord);
         #elif defined (X3D_BASE_TEXTURE_CUBE)
            vec4 textureColor = texture (x3d_BaseTexture .textureCube, texCoord);
         #endif

         vec4 textureColorComponents = textureColor .rgba;

         #if defined (X3D_COLORSPACE_SRGB)
            #if defined (X3D_BASE_TEXTURE_LINEAR)
               #if 1
                  textureColorComponents = linearToSRGB (textureColorComponents);
               #endif
            #endif
         #elif defined (X3D_COLORSPACE_LINEAR)
            #if !defined (X3D_BASE_TEXTURE_LINEAR)
               #if 1
                  textureColorComponents = sRGBToLinear (textureColorComponents);
               #endif
            #endif
         #endif

         return textureColorComponents;
      }
      #endif
      

vec4
getBaseColor ()
{
   // Get base parameter.

   float alpha     = 1.0 - x3d_Material .transparency;
   vec4  baseColor = vec4 (x3d_Material .baseColor, alpha);

   // In addition to the material properties, if a primitive specifies a vertex color using the attribute semantic property COLOR_0, then this value acts as an additional linear multiplier to base color.
   #if defined (X3D_COLOR_MATERIAL)
      baseColor *= color;
   #endif

   // Get texture color.

   #if defined (X3D_BASE_TEXTURE)
      baseColor *= getBaseTexture ();
   #elif defined (X3D_TEXTURE)
      baseColor = getTextureColor (baseColor, vec4 (vec3 (1.0), alpha));
   #endif

   return baseColor;
}



      #if defined (X3D_METALLIC_ROUGHNESS_TEXTURE)

      uniform x3d_MetallicRoughnessTextureParameters x3d_MetallicRoughnessTexture;

      vec4
      getMetallicRoughnessTexture ()
      {
         // Get texture color.

         vec3 texCoord = getTexCoord (x3d_MetallicRoughnessTexture .textureTransformMapping, x3d_MetallicRoughnessTexture .textureCoordinateMapping);

         #if defined (X3D_METALLIC_ROUGHNESS_TEXTURE_FLIP_Y)
            // Flip Y if needed. Must be done after.
            texCoord .y = 1.0 - texCoord .y;
         #endif

         #if defined (X3D_METALLIC_ROUGHNESS_TEXTURE_2D)
            vec4 textureColor = texture (x3d_MetallicRoughnessTexture .texture2D, texCoord .st);
         #elif defined (X3D_METALLIC_ROUGHNESS_TEXTURE_3D)
            vec4 textureColor = texture (x3d_MetallicRoughnessTexture .texture3D, texCoord);
         #elif defined (X3D_METALLIC_ROUGHNESS_TEXTURE_CUBE)
            vec4 textureColor = texture (x3d_MetallicRoughnessTexture .textureCube, texCoord);
         #endif

         vec4 textureColorComponents = textureColor .rgba;

         #if defined (X3D_COLORSPACE_SRGB)
            #if defined (X3D_METALLIC_ROUGHNESS_TEXTURE_LINEAR)
               #if 0
                  textureColorComponents = linearToSRGB (textureColorComponents);
               #endif
            #endif
         #elif defined (X3D_COLORSPACE_LINEAR)
            #if !defined (X3D_METALLIC_ROUGHNESS_TEXTURE_LINEAR)
               #if 0
                  textureColorComponents = sRGBToLinear (textureColorComponents);
               #endif
            #endif
         #endif

         return textureColorComponents;
      }
      #endif
      

MaterialInfo
getMetallicRoughnessInfo (in MaterialInfo info)
{
   // Metallic and Roughness material properties are packed together
   // In glTF, these factors can be specified by fixed scalar values
   // or from a metallic-roughness map
   info .metallic            = x3d_Material .metallic;
   info .perceptualRoughness = x3d_Material .roughness;

   // Get texture color.

   #if defined (X3D_METALLIC_ROUGHNESS_TEXTURE)
      // Roughness is stored in the 'g' channel, metallic is stored in the 'b' channel.
      // This layout intentionally reserves the 'r' channel for (optional) occlusion map data
      vec4 mrSample = getMetallicRoughnessTexture ();

      info .metallic            *= mrSample .b;
      info .perceptualRoughness *= mrSample .g;
   #endif

   return info;
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
      

#if defined (X3D_EMISSIVE_STRENGTH_MATERIAL_EXT)
   uniform float x3d_EmissiveStrengthEXT;
#endif

vec3
getEmissiveColor ()
{
   // Get emissive parameter.

   vec3 emissiveColor = x3d_Material .emissiveColor;

   #if defined (X3D_EMISSIVE_STRENGTH_MATERIAL_EXT)
      emissiveColor *= x3d_EmissiveStrengthEXT;
   #endif

   // Get texture color.

   #if defined (X3D_EMISSIVE_TEXTURE)
      emissiveColor *= getEmissiveTexture ();
   #endif

   return emissiveColor;
}



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

#if defined (X3D_IOR_MATERIAL_EXT)
uniform float x3d_IorEXT;

MaterialInfo
getIorInfo (in MaterialInfo info)
{
    info .f0_dielectric = vec3 (pow ((x3d_IorEXT - 1.0) / (x3d_IorEXT + 1.0), 2.0));
    info .ior           = x3d_IorEXT;

    return info;
}
#endif

#if defined (X3D_SHEEN_MATERIAL_EXT)



      #if defined (X3D_SHEEN_COLOR_TEXTURE_EXT)

      uniform x3d_SheenColorTextureParametersEXT x3d_SheenColorTextureEXT;

      vec3
      getSheenColorTextureEXT ()
      {
         // Get texture color.

         vec3 texCoord = getTexCoord (x3d_SheenColorTextureEXT .textureTransformMapping, x3d_SheenColorTextureEXT .textureCoordinateMapping);

         #if defined (X3D_SHEEN_COLOR_TEXTURE_EXT_FLIP_Y)
            // Flip Y if needed. Must be done after.
            texCoord .y = 1.0 - texCoord .y;
         #endif

         #if defined (X3D_SHEEN_COLOR_TEXTURE_EXT_2D)
            vec4 textureColor = texture (x3d_SheenColorTextureEXT .texture2D, texCoord .st);
         #elif defined (X3D_SHEEN_COLOR_TEXTURE_EXT_3D)
            vec4 textureColor = texture (x3d_SheenColorTextureEXT .texture3D, texCoord);
         #elif defined (X3D_SHEEN_COLOR_TEXTURE_EXT_CUBE)
            vec4 textureColor = texture (x3d_SheenColorTextureEXT .textureCube, texCoord);
         #endif

         vec3 textureColorComponents = textureColor .rgb;

         #if defined (X3D_COLORSPACE_SRGB)
            #if defined (X3D_SHEEN_COLOR_TEXTURE_EXT_LINEAR)
               #if 1
                  textureColorComponents = linearToSRGB (textureColorComponents);
               #endif
            #endif
         #elif defined (X3D_COLORSPACE_LINEAR)
            #if !defined (X3D_SHEEN_COLOR_TEXTURE_EXT_LINEAR)
               #if 1
                  textureColorComponents = sRGBToLinear (textureColorComponents);
               #endif
            #endif
         #endif

         return textureColorComponents;
      }
      #endif
      


      #if defined (X3D_SHEEN_ROUGHNESS_TEXTURE_EXT)

      uniform x3d_SheenRoughnessTextureParametersEXT x3d_SheenRoughnessTextureEXT;

      float
      getSheenRoughnessTextureEXT ()
      {
         // Get texture color.

         vec3 texCoord = getTexCoord (x3d_SheenRoughnessTextureEXT .textureTransformMapping, x3d_SheenRoughnessTextureEXT .textureCoordinateMapping);

         #if defined (X3D_SHEEN_ROUGHNESS_TEXTURE_EXT_FLIP_Y)
            // Flip Y if needed. Must be done after.
            texCoord .y = 1.0 - texCoord .y;
         #endif

         #if defined (X3D_SHEEN_ROUGHNESS_TEXTURE_EXT_2D)
            vec4 textureColor = texture (x3d_SheenRoughnessTextureEXT .texture2D, texCoord .st);
         #elif defined (X3D_SHEEN_ROUGHNESS_TEXTURE_EXT_3D)
            vec4 textureColor = texture (x3d_SheenRoughnessTextureEXT .texture3D, texCoord);
         #elif defined (X3D_SHEEN_ROUGHNESS_TEXTURE_EXT_CUBE)
            vec4 textureColor = texture (x3d_SheenRoughnessTextureEXT .textureCube, texCoord);
         #endif

         float textureColorComponents = textureColor .a;

         #if defined (X3D_COLORSPACE_SRGB)
            #if defined (X3D_SHEEN_ROUGHNESS_TEXTURE_EXT_LINEAR)
               #if 0
                  textureColorComponents = linearToSRGB (textureColorComponents);
               #endif
            #endif
         #elif defined (X3D_COLORSPACE_LINEAR)
            #if !defined (X3D_SHEEN_ROUGHNESS_TEXTURE_EXT_LINEAR)
               #if 0
                  textureColorComponents = sRGBToLinear (textureColorComponents);
               #endif
            #endif
         #endif

         return textureColorComponents;
      }
      #endif
      

uniform vec3  x3d_SheenColorEXT;
uniform float x3d_SheenRoughnessEXT;

MaterialInfo
getSheenInfo (in MaterialInfo info)
{
   info .sheenColorFactor     = x3d_SheenColorEXT;
   info .sheenRoughnessFactor = x3d_SheenRoughnessEXT;

   #if defined (X3D_SHEEN_COLOR_TEXTURE_EXT)
      info .sheenColorFactor *= getSheenColorTextureEXT ();
   #endif

   #if defined (X3D_SHEEN_ROUGHNESS_TEXTURE_EXT)
      info .sheenRoughnessFactor *= getSheenRoughnessTextureEXT ();
   #endif

   return info;
}

uniform sampler2D x3d_SheenELUTTextureEXT;

float
albedoSheenScalingLUT (const in float NdotV, const in float sheenRoughnessFactor)
{
   return texture (x3d_SheenELUTTextureEXT, vec2 (NdotV, sheenRoughnessFactor)) .r;
}
#endif

#if defined (X3D_DIFFUSE_TRANSMISSION_MATERIAL_EXT)



      #if defined (X3D_DIFFUSE_TRANSMISSION_TEXTURE_EXT)

      uniform x3d_DiffuseTransmissionTextureParametersEXT x3d_DiffuseTransmissionTextureEXT;

      float
      getDiffuseTransmissionTextureEXT ()
      {
         // Get texture color.

         vec3 texCoord = getTexCoord (x3d_DiffuseTransmissionTextureEXT .textureTransformMapping, x3d_DiffuseTransmissionTextureEXT .textureCoordinateMapping);

         #if defined (X3D_DIFFUSE_TRANSMISSION_TEXTURE_EXT_FLIP_Y)
            // Flip Y if needed. Must be done after.
            texCoord .y = 1.0 - texCoord .y;
         #endif

         #if defined (X3D_DIFFUSE_TRANSMISSION_TEXTURE_EXT_2D)
            vec4 textureColor = texture (x3d_DiffuseTransmissionTextureEXT .texture2D, texCoord .st);
         #elif defined (X3D_DIFFUSE_TRANSMISSION_TEXTURE_EXT_3D)
            vec4 textureColor = texture (x3d_DiffuseTransmissionTextureEXT .texture3D, texCoord);
         #elif defined (X3D_DIFFUSE_TRANSMISSION_TEXTURE_EXT_CUBE)
            vec4 textureColor = texture (x3d_DiffuseTransmissionTextureEXT .textureCube, texCoord);
         #endif

         float textureColorComponents = textureColor .a;

         #if defined (X3D_COLORSPACE_SRGB)
            #if defined (X3D_DIFFUSE_TRANSMISSION_TEXTURE_EXT_LINEAR)
               #if 0
                  textureColorComponents = linearToSRGB (textureColorComponents);
               #endif
            #endif
         #elif defined (X3D_COLORSPACE_LINEAR)
            #if !defined (X3D_DIFFUSE_TRANSMISSION_TEXTURE_EXT_LINEAR)
               #if 0
                  textureColorComponents = sRGBToLinear (textureColorComponents);
               #endif
            #endif
         #endif

         return textureColorComponents;
      }
      #endif
      


      #if defined (X3D_DIFFUSE_TRANSMISSION_COLOR_TEXTURE_EXT)

      uniform x3d_DiffuseTransmissionColorTextureParametersEXT x3d_DiffuseTransmissionColorTextureEXT;

      vec3
      getDiffuseTransmissionColorTextureEXT ()
      {
         // Get texture color.

         vec3 texCoord = getTexCoord (x3d_DiffuseTransmissionColorTextureEXT .textureTransformMapping, x3d_DiffuseTransmissionColorTextureEXT .textureCoordinateMapping);

         #if defined (X3D_DIFFUSE_TRANSMISSION_COLOR_TEXTURE_EXT_FLIP_Y)
            // Flip Y if needed. Must be done after.
            texCoord .y = 1.0 - texCoord .y;
         #endif

         #if defined (X3D_DIFFUSE_TRANSMISSION_COLOR_TEXTURE_EXT_2D)
            vec4 textureColor = texture (x3d_DiffuseTransmissionColorTextureEXT .texture2D, texCoord .st);
         #elif defined (X3D_DIFFUSE_TRANSMISSION_COLOR_TEXTURE_EXT_3D)
            vec4 textureColor = texture (x3d_DiffuseTransmissionColorTextureEXT .texture3D, texCoord);
         #elif defined (X3D_DIFFUSE_TRANSMISSION_COLOR_TEXTURE_EXT_CUBE)
            vec4 textureColor = texture (x3d_DiffuseTransmissionColorTextureEXT .textureCube, texCoord);
         #endif

         vec3 textureColorComponents = textureColor .rgb;

         #if defined (X3D_COLORSPACE_SRGB)
            #if defined (X3D_DIFFUSE_TRANSMISSION_COLOR_TEXTURE_EXT_LINEAR)
               #if 1
                  textureColorComponents = linearToSRGB (textureColorComponents);
               #endif
            #endif
         #elif defined (X3D_COLORSPACE_LINEAR)
            #if !defined (X3D_DIFFUSE_TRANSMISSION_COLOR_TEXTURE_EXT_LINEAR)
               #if 1
                  textureColorComponents = sRGBToLinear (textureColorComponents);
               #endif
            #endif
         #endif

         return textureColorComponents;
      }
      #endif
      

uniform float x3d_DiffuseTransmissionEXT;
uniform vec3  x3d_DiffuseTransmissionColorEXT;

MaterialInfo
getDiffuseTransmissionInfo (in MaterialInfo info)
{
   info .diffuseTransmissionFactor      = x3d_DiffuseTransmissionEXT;
   info .diffuseTransmissionColorFactor = x3d_DiffuseTransmissionColorEXT;

   #if defined (X3D_DIFFUSE_TRANSMISSION_TEXTURE_EXT)
      info .diffuseTransmissionFactor *= getDiffuseTransmissionTextureEXT ();
   #endif

   #if defined (X3D_DIFFUSE_TRANSMISSION_COLOR_TEXTURE_EXT)
      info .diffuseTransmissionColorFactor *= getDiffuseTransmissionColorTextureEXT ();
   #endif

   return info;
}
#endif

#if defined (X3D_VOLUME_SCATTER_MATERIAL_EXT)



      #if defined (X3D_SCATTER_STRENGTH_TEXTURE_EXT)

      uniform x3d_ScatterStrengthTextureParametersEXT x3d_ScatterStrengthTextureEXT;

      float
      getScatterStrengthTextureEXT ()
      {
         // Get texture color.

         vec3 texCoord = getTexCoord (x3d_ScatterStrengthTextureEXT .textureTransformMapping, x3d_ScatterStrengthTextureEXT .textureCoordinateMapping);

         #if defined (X3D_SCATTER_STRENGTH_TEXTURE_EXT_FLIP_Y)
            // Flip Y if needed. Must be done after.
            texCoord .y = 1.0 - texCoord .y;
         #endif

         #if defined (X3D_SCATTER_STRENGTH_TEXTURE_EXT_2D)
            vec4 textureColor = texture (x3d_ScatterStrengthTextureEXT .texture2D, texCoord .st);
         #elif defined (X3D_SCATTER_STRENGTH_TEXTURE_EXT_3D)
            vec4 textureColor = texture (x3d_ScatterStrengthTextureEXT .texture3D, texCoord);
         #elif defined (X3D_SCATTER_STRENGTH_TEXTURE_EXT_CUBE)
            vec4 textureColor = texture (x3d_ScatterStrengthTextureEXT .textureCube, texCoord);
         #endif

         float textureColorComponents = textureColor .a;

         #if defined (X3D_COLORSPACE_SRGB)
            #if defined (X3D_SCATTER_STRENGTH_TEXTURE_EXT_LINEAR)
               #if 1
                  textureColorComponents = linearToSRGB (textureColorComponents);
               #endif
            #endif
         #elif defined (X3D_COLORSPACE_LINEAR)
            #if !defined (X3D_SCATTER_STRENGTH_TEXTURE_EXT_LINEAR)
               #if 1
                  textureColorComponents = sRGBToLinear (textureColorComponents);
               #endif
            #endif
         #endif

         return textureColorComponents;
      }
      #endif
      


      #if defined (X3D_MULTISCATTER_COLOR_TEXTURE_EXT)

      uniform x3d_MultiscatterColorTextureParametersEXT x3d_MultiscatterColorTextureEXT;

      vec3
      getMultiscatterColorTextureEXT ()
      {
         // Get texture color.

         vec3 texCoord = getTexCoord (x3d_MultiscatterColorTextureEXT .textureTransformMapping, x3d_MultiscatterColorTextureEXT .textureCoordinateMapping);

         #if defined (X3D_MULTISCATTER_COLOR_TEXTURE_EXT_FLIP_Y)
            // Flip Y if needed. Must be done after.
            texCoord .y = 1.0 - texCoord .y;
         #endif

         #if defined (X3D_MULTISCATTER_COLOR_TEXTURE_EXT_2D)
            vec4 textureColor = texture (x3d_MultiscatterColorTextureEXT .texture2D, texCoord .st);
         #elif defined (X3D_MULTISCATTER_COLOR_TEXTURE_EXT_3D)
            vec4 textureColor = texture (x3d_MultiscatterColorTextureEXT .texture3D, texCoord);
         #elif defined (X3D_MULTISCATTER_COLOR_TEXTURE_EXT_CUBE)
            vec4 textureColor = texture (x3d_MultiscatterColorTextureEXT .textureCube, texCoord);
         #endif

         vec3 textureColorComponents = textureColor .rgb;

         #if defined (X3D_COLORSPACE_SRGB)
            #if defined (X3D_MULTISCATTER_COLOR_TEXTURE_EXT_LINEAR)
               #if 1
                  textureColorComponents = linearToSRGB (textureColorComponents);
               #endif
            #endif
         #elif defined (X3D_COLORSPACE_LINEAR)
            #if !defined (X3D_MULTISCATTER_COLOR_TEXTURE_EXT_LINEAR)
               #if 1
                  textureColorComponents = sRGBToLinear (textureColorComponents);
               #endif
            #endif
         #endif

         return textureColorComponents;
      }
      #endif
      

uniform float x3d_ScatterStrengthEXT;
uniform vec3 x3d_MultiscatterColorEXT;

MaterialInfo
getVolumeScatterInfo (in MaterialInfo info)
{
   // TODO: use scatter.
   float scatterStrength = x3d_ScatterStrengthEXT;

   #if defined (X3D_SCATTER_STRENGTH_TEXTURE_EXT)
      scatterStrength *= getScatterStrengthTextureEXT ();
   #endif

   info .multiscatterColor = x3d_MultiscatterColorEXT;

   #if defined (X3D_MULTISCATTER_COLOR_TEXTURE_EXT)
      info .multiscatterColor *= getMultiscatterColorTextureEXT ();
   #endif

   return info;
}
#endif

#if defined (X3D_CLEARCOAT_MATERIAL_EXT)



      #if defined (X3D_CLEARCOAT_TEXTURE_EXT)

      uniform x3d_ClearcoatTextureParametersEXT x3d_ClearcoatTextureEXT;

      float
      getClearcoatTextureEXT ()
      {
         // Get texture color.

         vec3 texCoord = getTexCoord (x3d_ClearcoatTextureEXT .textureTransformMapping, x3d_ClearcoatTextureEXT .textureCoordinateMapping);

         #if defined (X3D_CLEARCOAT_TEXTURE_EXT_FLIP_Y)
            // Flip Y if needed. Must be done after.
            texCoord .y = 1.0 - texCoord .y;
         #endif

         #if defined (X3D_CLEARCOAT_TEXTURE_EXT_2D)
            vec4 textureColor = texture (x3d_ClearcoatTextureEXT .texture2D, texCoord .st);
         #elif defined (X3D_CLEARCOAT_TEXTURE_EXT_3D)
            vec4 textureColor = texture (x3d_ClearcoatTextureEXT .texture3D, texCoord);
         #elif defined (X3D_CLEARCOAT_TEXTURE_EXT_CUBE)
            vec4 textureColor = texture (x3d_ClearcoatTextureEXT .textureCube, texCoord);
         #endif

         float textureColorComponents = textureColor .r;

         #if defined (X3D_COLORSPACE_SRGB)
            #if defined (X3D_CLEARCOAT_TEXTURE_EXT_LINEAR)
               #if 0
                  textureColorComponents = linearToSRGB (textureColorComponents);
               #endif
            #endif
         #elif defined (X3D_COLORSPACE_LINEAR)
            #if !defined (X3D_CLEARCOAT_TEXTURE_EXT_LINEAR)
               #if 0
                  textureColorComponents = sRGBToLinear (textureColorComponents);
               #endif
            #endif
         #endif

         return textureColorComponents;
      }
      #endif
      


      #if defined (X3D_CLEARCOAT_ROUGHNESS_TEXTURE_EXT)

      uniform x3d_ClearcoatRoughnessTextureParametersEXT x3d_ClearcoatRoughnessTextureEXT;

      float
      getClearcoatRoughnessTextureEXT ()
      {
         // Get texture color.

         vec3 texCoord = getTexCoord (x3d_ClearcoatRoughnessTextureEXT .textureTransformMapping, x3d_ClearcoatRoughnessTextureEXT .textureCoordinateMapping);

         #if defined (X3D_CLEARCOAT_ROUGHNESS_TEXTURE_EXT_FLIP_Y)
            // Flip Y if needed. Must be done after.
            texCoord .y = 1.0 - texCoord .y;
         #endif

         #if defined (X3D_CLEARCOAT_ROUGHNESS_TEXTURE_EXT_2D)
            vec4 textureColor = texture (x3d_ClearcoatRoughnessTextureEXT .texture2D, texCoord .st);
         #elif defined (X3D_CLEARCOAT_ROUGHNESS_TEXTURE_EXT_3D)
            vec4 textureColor = texture (x3d_ClearcoatRoughnessTextureEXT .texture3D, texCoord);
         #elif defined (X3D_CLEARCOAT_ROUGHNESS_TEXTURE_EXT_CUBE)
            vec4 textureColor = texture (x3d_ClearcoatRoughnessTextureEXT .textureCube, texCoord);
         #endif

         float textureColorComponents = textureColor .g;

         #if defined (X3D_COLORSPACE_SRGB)
            #if defined (X3D_CLEARCOAT_ROUGHNESS_TEXTURE_EXT_LINEAR)
               #if 0
                  textureColorComponents = linearToSRGB (textureColorComponents);
               #endif
            #endif
         #elif defined (X3D_COLORSPACE_LINEAR)
            #if !defined (X3D_CLEARCOAT_ROUGHNESS_TEXTURE_EXT_LINEAR)
               #if 0
                  textureColorComponents = sRGBToLinear (textureColorComponents);
               #endif
            #endif
         #endif

         return textureColorComponents;
      }
      #endif
      


      #if defined (X3D_CLEARCOAT_NORMAL_TEXTURE_EXT)

      uniform x3d_ClearcoatNormalTextureParametersEXT x3d_ClearcoatNormalTextureEXT;

      vec3
      getClearcoatNormalTextureEXT ()
      {
         // Get texture color.

         vec3 texCoord = getTexCoord (x3d_ClearcoatNormalTextureEXT .textureTransformMapping, x3d_ClearcoatNormalTextureEXT .textureCoordinateMapping);

         #if defined (X3D_CLEARCOAT_NORMAL_TEXTURE_EXT_FLIP_Y)
            // Flip Y if needed. Must be done after.
            texCoord .y = 1.0 - texCoord .y;
         #endif

         #if defined (X3D_CLEARCOAT_NORMAL_TEXTURE_EXT_2D)
            vec4 textureColor = texture (x3d_ClearcoatNormalTextureEXT .texture2D, texCoord .st);
         #elif defined (X3D_CLEARCOAT_NORMAL_TEXTURE_EXT_3D)
            vec4 textureColor = texture (x3d_ClearcoatNormalTextureEXT .texture3D, texCoord);
         #elif defined (X3D_CLEARCOAT_NORMAL_TEXTURE_EXT_CUBE)
            vec4 textureColor = texture (x3d_ClearcoatNormalTextureEXT .textureCube, texCoord);
         #endif

         vec3 textureColorComponents = textureColor .rgb;

         #if defined (X3D_COLORSPACE_SRGB)
            #if defined (X3D_CLEARCOAT_NORMAL_TEXTURE_EXT_LINEAR)
               #if 0
                  textureColorComponents = linearToSRGB (textureColorComponents);
               #endif
            #endif
         #elif defined (X3D_COLORSPACE_LINEAR)
            #if !defined (X3D_CLEARCOAT_NORMAL_TEXTURE_EXT_LINEAR)
               #if 0
                  textureColorComponents = sRGBToLinear (textureColorComponents);
               #endif
            #endif
         #endif

         return textureColorComponents;
      }
      #endif
      

#if defined (X3D_CLEARCOAT_NORMAL_TEXTURE_EXT)
const float x3d_ClearcoatNormalScaleEXT = 1.0;
#endif

vec3
getClearcoatNormalEXT (const in NormalInfo normalInfo)
{
   #if defined (X3D_CLEARCOAT_NORMAL_TEXTURE_EXT)
      vec3 color = getClearcoatNormalTextureEXT ();
      vec3 n     = color * 2.0 - vec3 (1.0);

      n *= vec3 (vec2 (x3d_ClearcoatNormalScaleEXT), 1.0);
      n  = mat3 (normalInfo .t, normalInfo .b, normalInfo .ng) * normalize (n);

      return n;
   #else
      return normalInfo .ng;
   #endif
}

uniform float x3d_ClearcoatEXT;
uniform float x3d_ClearcoatRoughnessEXT;

MaterialInfo
getClearCoatInfo (in MaterialInfo info, const in NormalInfo normalInfo)
{
   info .clearcoatFactor    = x3d_ClearcoatEXT;
   info .clearcoatRoughness = x3d_ClearcoatRoughnessEXT;
   info .clearcoatF0        = vec3 (pow ((info .ior - 1.0) / (info .ior + 1.0), 2.0));
   info .clearcoatF90       = vec3 (1.0);

   #if defined (X3D_CLEARCOAT_TEXTURE_EXT)
      info .clearcoatFactor *= getClearcoatTextureEXT ();
   #endif

   #if defined (X3D_CLEARCOAT_ROUGHNESS_TEXTURE_EXT)
      info .clearcoatRoughness *= getClearcoatRoughnessTextureEXT ();
   #endif

   info .clearcoatNormal    = getClearcoatNormalEXT (normalInfo);
   info .clearcoatRoughness = clamp (info .clearcoatRoughness, 0.0, 1.0);
   return info;
}
#endif

#if defined (X3D_SPECULAR_MATERIAL_EXT)



      #if defined (X3D_SPECULAR_TEXTURE_EXT)

      uniform x3d_SpecularTextureParametersEXT x3d_SpecularTextureEXT;

      float
      getSpecularTextureEXT ()
      {
         // Get texture color.

         vec3 texCoord = getTexCoord (x3d_SpecularTextureEXT .textureTransformMapping, x3d_SpecularTextureEXT .textureCoordinateMapping);

         #if defined (X3D_SPECULAR_TEXTURE_EXT_FLIP_Y)
            // Flip Y if needed. Must be done after.
            texCoord .y = 1.0 - texCoord .y;
         #endif

         #if defined (X3D_SPECULAR_TEXTURE_EXT_2D)
            vec4 textureColor = texture (x3d_SpecularTextureEXT .texture2D, texCoord .st);
         #elif defined (X3D_SPECULAR_TEXTURE_EXT_3D)
            vec4 textureColor = texture (x3d_SpecularTextureEXT .texture3D, texCoord);
         #elif defined (X3D_SPECULAR_TEXTURE_EXT_CUBE)
            vec4 textureColor = texture (x3d_SpecularTextureEXT .textureCube, texCoord);
         #endif

         float textureColorComponents = textureColor .a;

         #if defined (X3D_COLORSPACE_SRGB)
            #if defined (X3D_SPECULAR_TEXTURE_EXT_LINEAR)
               #if 0
                  textureColorComponents = linearToSRGB (textureColorComponents);
               #endif
            #endif
         #elif defined (X3D_COLORSPACE_LINEAR)
            #if !defined (X3D_SPECULAR_TEXTURE_EXT_LINEAR)
               #if 0
                  textureColorComponents = sRGBToLinear (textureColorComponents);
               #endif
            #endif
         #endif

         return textureColorComponents;
      }
      #endif
      


      #if defined (X3D_SPECULAR_COLOR_TEXTURE_EXT)

      uniform x3d_SpecularColorTextureParametersEXT x3d_SpecularColorTextureEXT;

      vec3
      getSpecularColorTextureEXT ()
      {
         // Get texture color.

         vec3 texCoord = getTexCoord (x3d_SpecularColorTextureEXT .textureTransformMapping, x3d_SpecularColorTextureEXT .textureCoordinateMapping);

         #if defined (X3D_SPECULAR_COLOR_TEXTURE_EXT_FLIP_Y)
            // Flip Y if needed. Must be done after.
            texCoord .y = 1.0 - texCoord .y;
         #endif

         #if defined (X3D_SPECULAR_COLOR_TEXTURE_EXT_2D)
            vec4 textureColor = texture (x3d_SpecularColorTextureEXT .texture2D, texCoord .st);
         #elif defined (X3D_SPECULAR_COLOR_TEXTURE_EXT_3D)
            vec4 textureColor = texture (x3d_SpecularColorTextureEXT .texture3D, texCoord);
         #elif defined (X3D_SPECULAR_COLOR_TEXTURE_EXT_CUBE)
            vec4 textureColor = texture (x3d_SpecularColorTextureEXT .textureCube, texCoord);
         #endif

         vec3 textureColorComponents = textureColor .rgb;

         #if defined (X3D_COLORSPACE_SRGB)
            #if defined (X3D_SPECULAR_COLOR_TEXTURE_EXT_LINEAR)
               #if 1
                  textureColorComponents = linearToSRGB (textureColorComponents);
               #endif
            #endif
         #elif defined (X3D_COLORSPACE_LINEAR)
            #if !defined (X3D_SPECULAR_COLOR_TEXTURE_EXT_LINEAR)
               #if 1
                  textureColorComponents = sRGBToLinear (textureColorComponents);
               #endif
            #endif
         #endif

         return textureColorComponents;
      }
      #endif
      

uniform float x3d_SpecularEXT;
uniform vec3  x3d_SpecularColorEXT;
uniform float x3d_SpecularStrengthEXT;

MaterialInfo
getSpecularInfo (in MaterialInfo info)
{
   vec3 specularColor   = x3d_SpecularStrengthEXT * x3d_SpecularColorEXT;
   vec4 specularTexture = vec4 (1.0);

   #if defined (X3D_SPECULAR_TEXTURE_EXT)
      specularTexture .a = getSpecularTextureEXT ();
   #endif

   #if defined (X3D_SPECULAR_COLOR_TEXTURE_EXT)
      specularTexture .rgb = getSpecularColorTextureEXT ();
   #endif

   info .f0_dielectric  = min (info .f0_dielectric * specularColor * specularTexture .rgb, vec3 (1.0));
   info .specularWeight = x3d_SpecularEXT * specularTexture .a;
   info .f90_dielectric = vec3 (info .specularWeight);

   return info;
}
#endif

#if defined (X3D_VOLUME_MATERIAL_EXT)



      #if defined (X3D_THICKNESS_TEXTURE_EXT)

      uniform x3d_ThicknessTextureParametersEXT x3d_ThicknessTextureEXT;

      float
      getThicknessTextureEXT ()
      {
         // Get texture color.

         vec3 texCoord = getTexCoord (x3d_ThicknessTextureEXT .textureTransformMapping, x3d_ThicknessTextureEXT .textureCoordinateMapping);

         #if defined (X3D_THICKNESS_TEXTURE_EXT_FLIP_Y)
            // Flip Y if needed. Must be done after.
            texCoord .y = 1.0 - texCoord .y;
         #endif

         #if defined (X3D_THICKNESS_TEXTURE_EXT_2D)
            vec4 textureColor = texture (x3d_ThicknessTextureEXT .texture2D, texCoord .st);
         #elif defined (X3D_THICKNESS_TEXTURE_EXT_3D)
            vec4 textureColor = texture (x3d_ThicknessTextureEXT .texture3D, texCoord);
         #elif defined (X3D_THICKNESS_TEXTURE_EXT_CUBE)
            vec4 textureColor = texture (x3d_ThicknessTextureEXT .textureCube, texCoord);
         #endif

         float textureColorComponents = textureColor .g;

         #if defined (X3D_COLORSPACE_SRGB)
            #if defined (X3D_THICKNESS_TEXTURE_EXT_LINEAR)
               #if 0
                  textureColorComponents = linearToSRGB (textureColorComponents);
               #endif
            #endif
         #elif defined (X3D_COLORSPACE_LINEAR)
            #if !defined (X3D_THICKNESS_TEXTURE_EXT_LINEAR)
               #if 0
                  textureColorComponents = sRGBToLinear (textureColorComponents);
               #endif
            #endif
         #endif

         return textureColorComponents;
      }
      #endif
      

uniform float x3d_ThicknessEXT;
uniform float x3d_AttenuationDistanceEXT;
uniform vec3  x3d_AttenuationColorEXT;

MaterialInfo
getVolumeInfo (in MaterialInfo info)
{
   info .thickness           = x3d_ThicknessEXT;
   info .attenuationDistance = x3d_AttenuationDistanceEXT;
   info .attenuationColor    = x3d_AttenuationColorEXT;

   #if defined (X3D_THICKNESS_TEXTURE_EXT)
      info .thickness *= getThicknessTextureEXT ();
   #endif

   return info;
}
#endif

#if defined (X3D_IRIDESCENCE_MATERIAL_EXT)



      #if defined (X3D_IRIDESCENCE_TEXTURE_EXT)

      uniform x3d_IridescenceTextureParametersEXT x3d_IridescenceTextureEXT;

      float
      getIridescenceTextureEXT ()
      {
         // Get texture color.

         vec3 texCoord = getTexCoord (x3d_IridescenceTextureEXT .textureTransformMapping, x3d_IridescenceTextureEXT .textureCoordinateMapping);

         #if defined (X3D_IRIDESCENCE_TEXTURE_EXT_FLIP_Y)
            // Flip Y if needed. Must be done after.
            texCoord .y = 1.0 - texCoord .y;
         #endif

         #if defined (X3D_IRIDESCENCE_TEXTURE_EXT_2D)
            vec4 textureColor = texture (x3d_IridescenceTextureEXT .texture2D, texCoord .st);
         #elif defined (X3D_IRIDESCENCE_TEXTURE_EXT_3D)
            vec4 textureColor = texture (x3d_IridescenceTextureEXT .texture3D, texCoord);
         #elif defined (X3D_IRIDESCENCE_TEXTURE_EXT_CUBE)
            vec4 textureColor = texture (x3d_IridescenceTextureEXT .textureCube, texCoord);
         #endif

         float textureColorComponents = textureColor .r;

         #if defined (X3D_COLORSPACE_SRGB)
            #if defined (X3D_IRIDESCENCE_TEXTURE_EXT_LINEAR)
               #if 0
                  textureColorComponents = linearToSRGB (textureColorComponents);
               #endif
            #endif
         #elif defined (X3D_COLORSPACE_LINEAR)
            #if !defined (X3D_IRIDESCENCE_TEXTURE_EXT_LINEAR)
               #if 0
                  textureColorComponents = sRGBToLinear (textureColorComponents);
               #endif
            #endif
         #endif

         return textureColorComponents;
      }
      #endif
      


      #if defined (X3D_IRIDESCENCE_THICKNESS_TEXTURE_EXT)

      uniform x3d_IridescenceThicknessTextureParametersEXT x3d_IridescenceThicknessTextureEXT;

      float
      getIridescenceThicknessTextureEXT ()
      {
         // Get texture color.

         vec3 texCoord = getTexCoord (x3d_IridescenceThicknessTextureEXT .textureTransformMapping, x3d_IridescenceThicknessTextureEXT .textureCoordinateMapping);

         #if defined (X3D_IRIDESCENCE_THICKNESS_TEXTURE_EXT_FLIP_Y)
            // Flip Y if needed. Must be done after.
            texCoord .y = 1.0 - texCoord .y;
         #endif

         #if defined (X3D_IRIDESCENCE_THICKNESS_TEXTURE_EXT_2D)
            vec4 textureColor = texture (x3d_IridescenceThicknessTextureEXT .texture2D, texCoord .st);
         #elif defined (X3D_IRIDESCENCE_THICKNESS_TEXTURE_EXT_3D)
            vec4 textureColor = texture (x3d_IridescenceThicknessTextureEXT .texture3D, texCoord);
         #elif defined (X3D_IRIDESCENCE_THICKNESS_TEXTURE_EXT_CUBE)
            vec4 textureColor = texture (x3d_IridescenceThicknessTextureEXT .textureCube, texCoord);
         #endif

         float textureColorComponents = textureColor .g;

         #if defined (X3D_COLORSPACE_SRGB)
            #if defined (X3D_IRIDESCENCE_THICKNESS_TEXTURE_EXT_LINEAR)
               #if 0
                  textureColorComponents = linearToSRGB (textureColorComponents);
               #endif
            #endif
         #elif defined (X3D_COLORSPACE_LINEAR)
            #if !defined (X3D_IRIDESCENCE_THICKNESS_TEXTURE_EXT_LINEAR)
               #if 0
                  textureColorComponents = sRGBToLinear (textureColorComponents);
               #endif
            #endif
         #endif

         return textureColorComponents;
      }
      #endif
      

uniform float x3d_IridescenceEXT;
uniform float x3d_IridescenceIndexOfRefractionEXT;
uniform float x3d_IridescenceThicknessMinimumEXT;
uniform float x3d_IridescenceThicknessMaximumEXT;

MaterialInfo
getIridescenceInfo (in MaterialInfo info)
{
   info .iridescenceFactor    = x3d_IridescenceEXT;
   info .iridescenceIor       = x3d_IridescenceIndexOfRefractionEXT;
   info .iridescenceThickness = x3d_IridescenceThicknessMaximumEXT;

   #if defined (X3D_IRIDESCENCE_TEXTURE_EXT)
      info .iridescenceFactor *= getIridescenceTextureEXT ();
   #endif

   #if defined (X3D_IRIDESCENCE_THICKNESS_TEXTURE_EXT)
      float thicknessSampled = getIridescenceThicknessTextureEXT ();
      float thickness        = mix (x3d_IridescenceThicknessMinimumEXT, x3d_IridescenceThicknessMaximumEXT, thicknessSampled);

      info .iridescenceThickness = thickness;
   #endif

   return info;
}
#endif

#if defined (X3D_TRANSMISSION_MATERIAL_EXT)



      #if defined (X3D_TRANSMISSION_TEXTURE_EXT)

      uniform x3d_TransmissionTextureParametersEXT x3d_TransmissionTextureEXT;

      float
      getTransmissionTextureEXT ()
      {
         // Get texture color.

         vec3 texCoord = getTexCoord (x3d_TransmissionTextureEXT .textureTransformMapping, x3d_TransmissionTextureEXT .textureCoordinateMapping);

         #if defined (X3D_TRANSMISSION_TEXTURE_EXT_FLIP_Y)
            // Flip Y if needed. Must be done after.
            texCoord .y = 1.0 - texCoord .y;
         #endif

         #if defined (X3D_TRANSMISSION_TEXTURE_EXT_2D)
            vec4 textureColor = texture (x3d_TransmissionTextureEXT .texture2D, texCoord .st);
         #elif defined (X3D_TRANSMISSION_TEXTURE_EXT_3D)
            vec4 textureColor = texture (x3d_TransmissionTextureEXT .texture3D, texCoord);
         #elif defined (X3D_TRANSMISSION_TEXTURE_EXT_CUBE)
            vec4 textureColor = texture (x3d_TransmissionTextureEXT .textureCube, texCoord);
         #endif

         float textureColorComponents = textureColor .r;

         #if defined (X3D_COLORSPACE_SRGB)
            #if defined (X3D_TRANSMISSION_TEXTURE_EXT_LINEAR)
               #if 0
                  textureColorComponents = linearToSRGB (textureColorComponents);
               #endif
            #endif
         #elif defined (X3D_COLORSPACE_LINEAR)
            #if !defined (X3D_TRANSMISSION_TEXTURE_EXT_LINEAR)
               #if 0
                  textureColorComponents = sRGBToLinear (textureColorComponents);
               #endif
            #endif
         #endif

         return textureColorComponents;
      }
      #endif
      

uniform float x3d_TransmissionEXT;

#if defined (X3D_DISPERSION_MATERIAL_EXT)
   uniform float x3d_DispersionEXT;
#endif

MaterialInfo
getTransmissionInfo (in MaterialInfo info)
{
   info .transmissionFactor = x3d_TransmissionEXT;

   #if defined (X3D_TRANSMISSION_TEXTURE_EXT)
      info .transmissionFactor *= getTransmissionTextureEXT ();
   #endif

   #if defined (X3D_DISPERSION_MATERIAL_EXT)
      info .dispersion = x3d_DispersionEXT;
   #else
      info .dispersion = 0.0;
   #endif

   return info;
}
#endif

#if defined (X3D_ANISOTROPY_MATERIAL_EXT)



      #if defined (X3D_ANISOTROPY_TEXTURE_EXT)

      uniform x3d_AnisotropyTextureParametersEXT x3d_AnisotropyTextureEXT;

      vec3
      getAnisotropyTextureEXT ()
      {
         // Get texture color.

         vec3 texCoord = getTexCoord (x3d_AnisotropyTextureEXT .textureTransformMapping, x3d_AnisotropyTextureEXT .textureCoordinateMapping);

         #if defined (X3D_ANISOTROPY_TEXTURE_EXT_FLIP_Y)
            // Flip Y if needed. Must be done after.
            texCoord .y = 1.0 - texCoord .y;
         #endif

         #if defined (X3D_ANISOTROPY_TEXTURE_EXT_2D)
            vec4 textureColor = texture (x3d_AnisotropyTextureEXT .texture2D, texCoord .st);
         #elif defined (X3D_ANISOTROPY_TEXTURE_EXT_3D)
            vec4 textureColor = texture (x3d_AnisotropyTextureEXT .texture3D, texCoord);
         #elif defined (X3D_ANISOTROPY_TEXTURE_EXT_CUBE)
            vec4 textureColor = texture (x3d_AnisotropyTextureEXT .textureCube, texCoord);
         #endif

         vec3 textureColorComponents = textureColor .rgb;

         #if defined (X3D_COLORSPACE_SRGB)
            #if defined (X3D_ANISOTROPY_TEXTURE_EXT_LINEAR)
               #if 0
                  textureColorComponents = linearToSRGB (textureColorComponents);
               #endif
            #endif
         #elif defined (X3D_COLORSPACE_LINEAR)
            #if !defined (X3D_ANISOTROPY_TEXTURE_EXT_LINEAR)
               #if 0
                  textureColorComponents = sRGBToLinear (textureColorComponents);
               #endif
            #endif
         #endif

         return textureColorComponents;
      }
      #endif
      

uniform vec3 x3d_AnisotropyEXT;

MaterialInfo
getAnisotropyInfo (in MaterialInfo info, const in NormalInfo normalInfo)
{
   vec2  direction      = vec2 (1.0, 0.0);
   float strengthFactor = 1.0;

   #if defined (X3D_ANISOTROPY_TEXTURE_EXT)
      vec3 anisotropySample = getAnisotropyTextureEXT ();

      direction      = anisotropySample .xy * 2.0 - vec2 (1.0);
      strengthFactor = anisotropySample .z;
   #endif

   vec2 directionRotation = x3d_AnisotropyEXT .xy; // cos(theta), sin(theta)
   mat2 rotationMatrix    = mat2 (directionRotation .xy, -directionRotation .y, directionRotation .x);

   direction = rotationMatrix * direction;

   info .anisotropicT       = mat3 (normalInfo .t, normalInfo .b, normalInfo .n) * normalize (vec3 (direction, 0.0));
   info .anisotropicB       = cross (normalInfo .ng, info .anisotropicT);
   info .anisotropyStrength = clamp (x3d_AnisotropyEXT .z * strengthFactor, 0.0, 1.0);

   return info;
}
#endif

#if defined (X3D_RETROREFLECTION_MATERIAL_EXT)



      #if defined (X3D_RETROREFLECTION_TEXTURE_EXT)

      uniform x3d_RetroreflectionTextureParametersEXT x3d_RetroreflectionTextureEXT;

      float
      getRetroreflectionTextureEXT ()
      {
         // Get texture color.

         vec3 texCoord = getTexCoord (x3d_RetroreflectionTextureEXT .textureTransformMapping, x3d_RetroreflectionTextureEXT .textureCoordinateMapping);

         #if defined (X3D_RETROREFLECTION_TEXTURE_EXT_FLIP_Y)
            // Flip Y if needed. Must be done after.
            texCoord .y = 1.0 - texCoord .y;
         #endif

         #if defined (X3D_RETROREFLECTION_TEXTURE_EXT_2D)
            vec4 textureColor = texture (x3d_RetroreflectionTextureEXT .texture2D, texCoord .st);
         #elif defined (X3D_RETROREFLECTION_TEXTURE_EXT_3D)
            vec4 textureColor = texture (x3d_RetroreflectionTextureEXT .texture3D, texCoord);
         #elif defined (X3D_RETROREFLECTION_TEXTURE_EXT_CUBE)
            vec4 textureColor = texture (x3d_RetroreflectionTextureEXT .textureCube, texCoord);
         #endif

         float textureColorComponents = textureColor .r;

         #if defined (X3D_COLORSPACE_SRGB)
            #if defined (X3D_RETROREFLECTION_TEXTURE_EXT_LINEAR)
               #if 0
                  textureColorComponents = linearToSRGB (textureColorComponents);
               #endif
            #endif
         #elif defined (X3D_COLORSPACE_LINEAR)
            #if !defined (X3D_RETROREFLECTION_TEXTURE_EXT_LINEAR)
               #if 0
                  textureColorComponents = sRGBToLinear (textureColorComponents);
               #endif
            #endif
         #endif

         return textureColorComponents;
      }
      #endif
      

uniform float x3d_RetroreflectionEXT;

MaterialInfo
getRetroreflectionInfo (in MaterialInfo info)
{
   float retroreflectionFactor = x3d_RetroreflectionEXT;

   #if defined (X3D_RETROREFLECTION_TEXTURE_EXT)
      retroreflectionFactor *= getRetroreflectionTextureEXT ();
   #endif

   info .retroreflectionFactor = retroreflectionFactor;

   return info;
}
#endif
