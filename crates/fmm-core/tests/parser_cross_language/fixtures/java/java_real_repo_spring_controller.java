
import org.springframework.web.bind.annotation.RestController;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.PostMapping;
import java.util.List;

@RestController
public class UserController {
    @GetMapping
    public List<String> getUsers() {
        return List.of("alice", "bob");
    }

    @PostMapping
    public String createUser(String name) {
        return name;
    }

    private void validate(String name) {
        if (name == null) throw new IllegalArgumentException();
    }
}
