extern "C" {
    void interrupt_execution();
    void should_not_be_called();
}


extern "C" int crashme() {
    interrupt_execution();
    should_not_be_called();

    return 2;
}