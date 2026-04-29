# keep kotlinx-serialization
-keepattributes *Annotation*, InnerClasses
-dontnote kotlinx.serialization.AnnotationsKt
-keep,includedescriptorclasses class com.cinghialapp.**$$serializer { *; }
-keepclassmembers class com.cinghialapp.** {
    *** Companion;
}
-keepclasseswithmembers class com.cinghialapp.** {
    kotlinx.serialization.KSerializer serializer(...);
}
