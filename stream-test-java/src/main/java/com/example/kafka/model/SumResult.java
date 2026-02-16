package com.example.kafka.model;

public class SumResult {
    private long sum;

    public SumResult() {}

    public SumResult(long sum) {
        this.sum = sum;
    }

    public long getSum() {
        return sum;
    }

    public void setSum(long sum) {
        this.sum = sum;
    }

    @Override
    public String toString() {
        return "SumResult{sum=" + sum + "}";
    }
}
