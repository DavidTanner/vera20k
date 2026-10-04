// Replay one VERA20k annotation pass (a JSON ledger) onto a Ghidra program.
// Usage and ledger format: ../ghidra_pass.md
// Arguments: <ledger.json> [check|apply]  (no arguments: asks for both)
// @category VERA20k

import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import ghidra.app.cmd.function.CreateFunctionCmd;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.CommentType;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.Instruction;
import ghidra.program.model.symbol.SourceType;
import ghidra.program.model.symbol.Symbol;
import ghidra.program.model.symbol.SymbolType;

import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.ArrayList;
import java.util.List;

public class ApplyGhidraPass extends GhidraScript {
    enum State { PENDING, DONE, CONFLICT }

    record Status(State state, String detail) {}

    private String tag;

    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        File ledgerFile = args.length > 0 ? new File(args[0]) : askFile("Pass ledger", "Open");
        String mode = args.length > 1 ? args[1]
            : askChoice("Mode", "check reports only; apply writes", List.of("check", "apply"), "check");
        if (!mode.equals("check") && !mode.equals("apply")) {
            throw new IllegalArgumentException("mode must be check or apply, not " + mode);
        }
        JsonObject ledger = JsonParser.parseString(
            Files.readString(ledgerFile.toPath(), StandardCharsets.UTF_8)).getAsJsonObject();
        if (ledger.get("format").getAsInt() != 1) {
            throw new IllegalArgumentException("unsupported ledger format " + ledger.get("format"));
        }
        String expected = ledger.get("program_sha256").getAsString();
        String actual = currentProgram.getExecutableSHA256();
        if (actual == null || !actual.equalsIgnoreCase(expected)) {
            throw new IllegalStateException("program SHA-256 " + actual + " is not the ledger's " + expected);
        }
        tag = ledger.get("tag").getAsString();
        JsonArray ops = ledger.getAsJsonArray("ops");
        println("PASS " + ledger.get("pass").getAsString() + " (" + ops.size() + " ops, mode " + mode + ")");

        List<Status> before = new ArrayList<>();
        int pending = 0, done = 0, conflicts = 0;
        for (int i = 0; i < ops.size(); i++) {
            JsonObject op = ops.get(i).getAsJsonObject();
            Status status = evaluate(op);
            before.add(status);
            switch (status.state()) {
                case PENDING -> pending++;
                case DONE -> done++;
                case CONFLICT -> conflicts++;
            }
            if (status.state() != State.DONE || mode.equals("check")) {
                println(String.format("OP %d %s %s %s %s", i, op.get("op").getAsString(),
                    op.get("address").getAsString(), status.state(), status.detail()));
            }
        }
        println(String.format("SUMMARY pending=%d done=%d conflict=%d", pending, done, conflicts));
        if (mode.equals("check") || pending == 0) return;
        if (conflicts > 0 && ledger.has("atomic") && ledger.get("atomic").getAsBoolean()) {
            throw new IllegalStateException("atomic pass has conflicts; nothing applied");
        }

        int tx = currentProgram.startTransaction(ledger.get("pass").getAsString());
        boolean ok = false;
        int applied = 0;
        try {
            for (int i = 0; i < ops.size(); i++) {
                if (before.get(i).state() != State.PENDING) continue;
                JsonObject op = ops.get(i).getAsJsonObject();
                apply(op);
                Status after = evaluate(op);
                if (after.state() != State.DONE) {
                    throw new IllegalStateException("op " + i + " did not read back: " + after.detail());
                }
                applied++;
            }
            ok = true;
        } finally {
            currentProgram.endTransaction(tx, ok);
        }
        println(String.format("APPLIED %d (skipped conflicts: %d); save the program to keep them", applied, conflicts));
    }

    private Address address(JsonObject op) {
        return toAddr(Long.parseLong(op.get("address").getAsString().replaceFirst("^0[xX]", ""), 16));
    }

    private String plateParagraph(JsonObject op) {
        return tag + " " + op.get("text").getAsString();
    }

    private String plateAt(Address a) {
        Function f = getFunctionAt(a);
        String plate = f != null ? f.getComment() : currentProgram.getListing().getComment(CommentType.PLATE, a);
        return plate == null ? "" : plate;
    }

    private Status evaluate(JsonObject op) {
        Address a = address(op);
        switch (op.get("op").getAsString()) {
            case "rename_function": {
                Function f = getFunctionAt(a);
                if (f == null) return new Status(State.CONFLICT, "no function here");
                String from = op.get("from").getAsString();
                String to = op.get("to").getAsString();
                if (f.getName().equals(to)) return new Status(State.DONE, to);
                if (!f.getName().equals(from)) return new Status(State.CONFLICT, "current name " + f.getName());
                for (Symbol s : currentProgram.getSymbolTable().getSymbols(to)) {
                    if (s.getSymbolType() == SymbolType.FUNCTION && !s.getAddress().equals(a)) {
                        return new Status(State.CONFLICT, to + " already names " + s.getAddress());
                    }
                }
                return new Status(State.PENDING, from + " -> " + to);
            }
            case "append_plate": {
                String plate = plateAt(a);
                if (plate.contains(plateParagraph(op))) return new Status(State.DONE, "plate has the paragraph");
                if (plate.contains(tag)) return new Status(State.CONFLICT, "plate has a different " + tag + " paragraph");
                return new Status(State.PENDING, "append " + tag + " paragraph");
            }
            case "create_function": {
                String name = op.get("name").getAsString();
                Function f = getFunctionAt(a);
                if (f != null) {
                    return f.getName().equals(name) ? new Status(State.DONE, name)
                        : new Status(State.CONFLICT, "function " + f.getName() + " already starts here");
                }
                Function container = getFunctionContaining(a);
                if (container != null) return new Status(State.CONFLICT, "inside " + container.getName());
                Instruction ins = getInstructionAt(a);
                if (ins == null) return new Status(State.CONFLICT, "no instruction starts here");
                return new Status(State.PENDING, "create " + name);
            }
            default:
                return new Status(State.CONFLICT, "unknown op " + op.get("op"));
        }
    }

    private void apply(JsonObject op) throws Exception {
        Address a = address(op);
        switch (op.get("op").getAsString()) {
            case "rename_function" ->
                getFunctionAt(a).setName(op.get("to").getAsString(), SourceType.USER_DEFINED);
            case "append_plate" -> {
                String old = plateAt(a);
                String text = old.isEmpty() ? plateParagraph(op) : old + "\n\n" + plateParagraph(op);
                Function f = getFunctionAt(a);
                if (f != null) f.setComment(text);
                else currentProgram.getListing().setComment(a, CommentType.PLATE, text);
            }
            case "create_function" -> {
                CreateFunctionCmd cmd = new CreateFunctionCmd(
                    op.get("name").getAsString(), a, null, SourceType.USER_DEFINED);
                if (!cmd.applyTo(currentProgram, monitor)) {
                    throw new IllegalStateException("create function failed: " + cmd.getStatusMsg());
                }
            }
            default -> throw new IllegalArgumentException("unknown op " + op.get("op"));
        }
    }
}
