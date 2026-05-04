# keep kotlinx-serialization
-keepattributes *Annotation*, InnerClasses
-dontnote kotlinx.serialization.AnnotationsKt
-keep,includedescriptorclasses class com.dungeonsandapps.**$$serializer { *; }
-keepclassmembers class com.dungeonsandapps.** {
    *** Companion;
}
-keepclasseswithmembers class com.dungeonsandapps.** {
    kotlinx.serialization.KSerializer serializer(...);
}
