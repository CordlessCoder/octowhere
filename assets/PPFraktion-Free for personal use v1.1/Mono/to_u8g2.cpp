#include <inttypes.h>
#include "./ppfraktionmono_regular_24.c"
#include <iostream>

int main(void) { std::cout.write(reinterpret_cast<const char*>(bdf_font), sizeof(bdf_font) - 1); }
