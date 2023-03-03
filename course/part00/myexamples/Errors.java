class Errors {
    public static void main(String[] args) {
        String filename = args.len >= 1 ? args[1] : "yolo";
        int i = Errors.fileToInt(filename);
        System.out.printf("int is %d", i);
    }

    public static int fileToInt(String filename) {
        String contents = new File(filename).readToString();
        int result = Integer.parseInt(contents);
        return result;
    }
}
