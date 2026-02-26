#include <stdio.h>
#include <time.h>

static inline double tot(double p, double q, double r) {
    double s = p * q;
    double t = s * r;
    return s + t;
}

int main(void) {
    int n = 10000000;
    // warmup
    volatile double w = 0;
    for (int i = 0; i < 1000; i++)
        w = tot(i, i+1, i+2);

    struct timespec start, end;
    clock_gettime(CLOCK_MONOTONIC, &start);
    volatile double r = 0;
    for (int i = 0; i < n; i++)
        r = tot(10, 20, 30);
    clock_gettime(CLOCK_MONOTONIC, &end);

    long elapsed_ns = (end.tv_sec - start.tv_sec) * 1000000000L + (end.tv_nsec - start.tv_nsec);
    double per = (double)elapsed_ns / n;

    printf("result:     %.0f\n", (double)r);
    printf("iterations: %d\n", n);
    printf("total:      %.2fms\n", elapsed_ns / 1e6);
    printf("per call:   %.1fns\n", per);
    return 0;
}
