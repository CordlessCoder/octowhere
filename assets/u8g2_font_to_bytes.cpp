#include <inttypes.h>
#include <iostream>
#include "./marathon_shapiro_20o.c"

int main(void) { std::cout.write(reinterpret_cast<const char*>(bdf_font), sizeof(bdf_font) - 1); }
